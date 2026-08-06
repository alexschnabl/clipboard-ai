import asyncio
import re
import subprocess

import httpx
from claude_agent_sdk import AssistantMessage, ClaudeAgentOptions, ResultMessage, TextBlock, query

SYSTEM_PROMPT = (
    "Fix grammar and spelling. Return only the corrected text, "
    "with no preamble, explanation, or commentary."
)

_THINK_BLOCK = re.compile(r"<think>.*?</think>", re.DOTALL | re.IGNORECASE)
_UNCLOSED_THINK = re.compile(r"<think\b", re.IGNORECASE)


class BackendRefusal(Exception):
    """The backend produced nothing usable. Never paste after this."""


def strip_think_blocks(text: str) -> str:
    """Remove <think>...</think> spans emitted by reasoning models.

    Raises BackendRefusal if an unclosed <think tag remains after stripping,
    indicating a truncated reasoning model response.
    """
    stripped = _THINK_BLOCK.sub("", text)
    if _UNCLOSED_THINK.search(stripped):
        raise BackendRefusal("unclosed <think tag detected; response may be truncated")
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
        content = choices[0].get("message", {}).get("content") or ""
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
