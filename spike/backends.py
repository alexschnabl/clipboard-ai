import asyncio
import concurrent.futures
import re
import subprocess
import threading

import httpx
from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
    query,
)

SYSTEM_PROMPT = (
    "Fix grammar and spelling. Return only the corrected text, "
    "with no preamble, explanation, or commentary."
)

# Matches <think>...</think> and <thinking>...</thinking> (matched pairs
# only, via the \1 backreference on the tag name). Some reasoning models emit
# "thinking" as the tag name rather than "think"; both are covered here so
# neither shape of reasoning markup can reach the paste step.
_THINK_BLOCK = re.compile(r"<(think|thinking)>.*?</\1>", re.DOTALL | re.IGNORECASE)
# A residual opening OR closing tag of either spelling means stripping above
# left a fragment behind -- e.g. a truncated response missing its close, or a
# chat template that pre-injects the opening tag so the model's own output
# contains only the closing tag (a documented deployment pattern for
# reasoning models). Either shape must refuse, not paste the fragment.
_RESIDUAL_OPEN_THINK = re.compile(r"<(?:think|thinking)\b", re.IGNORECASE)
_RESIDUAL_CLOSE_THINK = re.compile(r"</(?:think|thinking)>", re.IGNORECASE)


class BackendRefusal(Exception):
    """The backend produced nothing usable. Never paste after this."""


def strip_think_blocks(text: str) -> str:
    """Remove <think>...</think> and <thinking>...</thinking> spans emitted
    by reasoning models.

    Raises BackendRefusal if a residual opening or closing think/thinking tag
    remains after stripping well-formed pairs, indicating a truncated
    response or a template that injects only one half of the tag pair.
    """
    stripped = _THINK_BLOCK.sub("", text)
    if _RESIDUAL_OPEN_THINK.search(stripped) or _RESIDUAL_CLOSE_THINK.search(stripped):
        raise BackendRefusal("residual think/thinking tag detected; response may be truncated")
    return stripped


def _default_http_post(url: str, payload: dict) -> dict:
    response = httpx.post(url, json=payload, timeout=120.0)
    response.raise_for_status()
    return response.json()


class OllamaBackend:
    """Local Ollama through its OpenAI-compatible chat completions endpoint."""

    name = "ollama"

    def __init__(
        self,
        base_url: str = "http://localhost:11434/v1",
        model: str = "qwen2.5:32b-instruct",
        http_post=None,
    ):
        self._url = f"{base_url.rstrip('/')}/chat/completions"
        self._model = model
        self._http_post = http_post or _default_http_post

    def transform(self, text: str) -> str:
        payload = self._http_post(
            self._url,
            {
                "model": self._model,
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": text},
                ],
                "stream": False,
            },
        )
        choices = payload.get("choices") or []
        if not choices:
            raise BackendRefusal(f"ollama returned no choices: {payload}")
        choice = choices[0] or {}
        content = (choice.get("message") or {}).get("content") or ""
        content = strip_think_blocks(content)
        if not content.strip():
            raise BackendRefusal("ollama returned empty content")
        return content.strip()


def _run_claude_cli(argv: list[str], input_text: str) -> str:
    try:
        result = subprocess.run(
            argv,
            input=input_text,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except subprocess.TimeoutExpired as e:
        raise BackendRefusal(f"claude timeout after {e.timeout}s") from e
    except FileNotFoundError as e:
        raise BackendRefusal(f"claude executable not found") from e
    if result.returncode != 0:
        raise BackendRefusal(f"claude exited {result.returncode}: {result.stderr}")
    return result.stdout


class ClaudeCLIBackend:
    """Cold `claude -p` subprocess per request. The latency baseline."""

    name = "claude-cli"

    def __init__(self, runner=None):
        self._runner = runner or _run_claude_cli

    def transform(self, text: str) -> str:
        output = self._runner(
            ["claude", "-p", "--system-prompt", SYSTEM_PROMPT],
            text,
        )
        if not output.strip():
            raise BackendRefusal("claude returned no output")
        return output.strip()


async def _run_agent_sdk_query(system_prompt: str, user_text: str) -> str:
    """Drive claude_agent_sdk.query() for a single exchange and collect its text.

    tools=[] disables the built-in toolset (Read/Bash/Edit/...) entirely.
    This backend only rewrites text, so it has no business invoking tools;
    disabling them removes the risk of a headless call hanging behind an
    interactive permission prompt and any chance of the model reaching for
    the filesystem mid-request.
    """
    options = ClaudeAgentOptions(
        system_prompt=system_prompt,
        tools=[],
    )
    chunks: list[str] = []
    async for message in query(prompt=user_text, options=options):
        if isinstance(message, AssistantMessage):
            if message.error is not None:
                raise BackendRefusal(f"agent sdk assistant error: {message.error}")
            for block in message.content:
                if isinstance(block, TextBlock):
                    chunks.append(block.text)
        elif isinstance(message, ResultMessage) and message.is_error:
            raise BackendRefusal(
                f"agent sdk result error: subtype={message.subtype} errors={message.errors}"
            )
    return "".join(chunks)


def _default_run_query(system_prompt: str, user_text: str) -> str:
    """Single-shot query through the Claude Agent SDK.

    Uses the existing Claude Code login rather than an API key.
    Written against https://code.claude.com/docs/en/agent-sdk — see Step 1.
    """
    return asyncio.run(_run_agent_sdk_query(system_prompt, user_text))


class AgentSDKBackend:
    """Claude Agent SDK, authenticating off the existing Claude Code login."""

    name = "agent-sdk"

    def __init__(self, run_query=None):
        self._run_query = run_query or _default_run_query

    def transform(self, text: str) -> str:
        output = self._run_query(SYSTEM_PROMPT, text)
        if not output.strip():
            raise BackendRefusal("agent sdk returned no output")
        return output.strip()


# Matches the httpx timeout=120.0 in _default_http_post and the
# subprocess.run timeout=120 in _run_claude_cli -- every backend bounds how
# long it will wait before giving up, so a hung session can't hold the
# `busy` lock in main.py forever and kill the hotkey for the rest of a run.
_SESSION_RESULT_TIMEOUT = 120.0


class _AgentSDKSession:
    """Owns one connect()ed ClaudeSDKClient on a dedicated background event
    loop thread, so repeated .ask() calls reuse the same underlying `claude`
    CLI subprocess and conversation session instead of spawning a fresh one
    per call. This is the warm-session path claude_agent_sdk.query() cannot
    exercise: query() is documented as "Stateless: each query is
    independent" and spawns a fresh subprocess every call.

    ClaudeSDKClient's own docs warn an instance cannot be used across
    different async runtime contexts: its internal task group is bound to
    whichever event loop was running when connect() was awaited. A single
    persistent event loop on a dedicated thread, driven from the synchronous
    .ask() caller via asyncio.run_coroutine_threadsafe, keeps connect() and
    every subsequent query()/receive_response() call on that same loop for
    the life of this object.
    """

    def __init__(self, system_prompt: str):
        self._loop = asyncio.new_event_loop()
        self._thread = threading.Thread(target=self._loop.run_forever, daemon=True)
        self._thread.start()
        self._client = ClaudeSDKClient(
            ClaudeAgentOptions(system_prompt=system_prompt, tools=[])
        )
        self._run(self._client.connect())

    def _run(self, coro):
        future = asyncio.run_coroutine_threadsafe(coro, self._loop)
        try:
            return future.result(timeout=_SESSION_RESULT_TIMEOUT)
        except concurrent.futures.TimeoutError as e:
            raise BackendRefusal(
                f"agent sdk session timed out after {_SESSION_RESULT_TIMEOUT}s"
            ) from e

    def ask(self, user_text: str) -> str:
        return self._run(self._ask(user_text))

    async def _ask(self, user_text: str) -> str:
        await self._client.query(user_text)
        chunks: list[str] = []
        async for message in self._client.receive_response():
            if isinstance(message, AssistantMessage):
                if message.error is not None:
                    raise BackendRefusal(
                        f"agent sdk session assistant error: {message.error}"
                    )
                for block in message.content:
                    if isinstance(block, TextBlock):
                        chunks.append(block.text)
            elif isinstance(message, ResultMessage) and message.is_error:
                raise BackendRefusal(
                    f"agent sdk session result error: "
                    f"subtype={message.subtype} errors={message.errors}"
                )
        return "".join(chunks)


class AgentSDKSessionBackend:
    """Claude Agent SDK with a single ClaudeSDKClient held open across
    .transform() calls, via ClaudeSDKClient (the persistent-session API),
    not query() (documented stateless, fresh subprocess per call).

    Answers the spike's third question: whether a warm session is
    meaningfully faster than a cold claude -p / query() call per request.
    The session connects lazily on the first .transform() call rather than
    in __init__, so constructing the backend has no side effects, matching
    the other backends -- and so a `--backend` choice that is never used
    never spawns a `claude` subprocess.
    """

    name = "agent-sdk-session"

    def __init__(self, run_query=None):
        self._run_query = run_query
        self._session: _AgentSDKSession | None = None

    def transform(self, text: str) -> str:
        run_query = self._run_query or self._session_ask
        output = run_query(text)
        if not output.strip():
            raise BackendRefusal("agent sdk session returned no output")
        return output.strip()

    def _session_ask(self, text: str) -> str:
        if self._session is None:
            self._session = _AgentSDKSession(SYSTEM_PROMPT)
        return self._session.ask(text)
