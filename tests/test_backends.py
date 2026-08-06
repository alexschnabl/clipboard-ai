import asyncio
import subprocess
import threading

import pytest

from spike.backends import SYSTEM_PROMPT, BackendRefusal, OllamaBackend, ClaudeCLIBackend, _run_claude_cli


def chat_response(content: str) -> dict:
    return {"choices": [{"message": {"role": "assistant", "content": content}}]}


def test_returns_the_message_content():
    backend = OllamaBackend(http_post=lambda url, payload: chat_response("Corrected."))
    assert backend.transform("corected") == "Corrected."


def test_strips_think_blocks():
    raw = "<think>The user wants grammar fixed.</think>Corrected."
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    assert backend.transform("corected") == "Corrected."


def test_raises_when_response_has_no_choices():
    backend = OllamaBackend(http_post=lambda url, payload: {"choices": []})
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_raises_when_content_is_only_a_think_block():
    raw = "<think>thinking out loud</think>   "
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_sends_the_expected_payload():
    seen = {}

    def http_post(url, payload):
        seen["url"] = url
        seen["payload"] = payload
        return chat_response("ok")

    OllamaBackend(http_post=http_post).transform("input text")
    assert seen["url"] == "http://localhost:11434/v1/chat/completions"
    assert seen["payload"]["model"] == "qwen2.5:32b-instruct"
    assert seen["payload"]["stream"] is False
    assert seen["payload"]["messages"] == [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": "input text"},
    ]


def test_strips_multiple_think_blocks():
    raw = "<think>First thought</think>Start<think>Second thought</think>End"
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    assert backend.transform("x") == "StartEnd"


def test_strips_mixed_case_think_blocks():
    raw = "<THINK>uppercase</THINK>Middle<Think>mixed</Think>End"
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    assert backend.transform("x") == "MiddleEnd"


def test_raises_when_unclosed_think_tag_present():
    raw = "<think>the user wants grammar fixed and I am still reasoning without a closing tag"
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_raises_when_response_truncated_mid_open_tag():
    # The exact shape a reasoning model produces when cut off by a token or
    # context limit mid-tag: no literal ">" ever arrives. A guard that
    # requires one would let this straight through to the paste.
    raw = "Corrected text.<think"
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_raises_when_open_think_tag_has_attributes():
    raw = 'Corrected.<think foo="1">reasoning'
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_raises_when_open_think_tag_followed_by_newline():
    raw = "<think\nstill reasoning"
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_raises_when_orphan_closing_think_tag_present():
    # Some chat templates pre-inject the opening <think> tag themselves, so the
    # model's own output only ever contains the closing tag. If that closing
    # tag survives (e.g. because the template's opening tag never reached this
    # code), the fragment must still be refused rather than pasted.
    raw = "The user wants grammar fixed. …</think>Corrected."
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_strips_thinking_block_variant():
    raw = "<thinking>reasoning about grammar</thinking>Corrected."
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    assert backend.transform("x") == "Corrected."


def test_raises_when_orphan_closing_thinking_tag_present():
    raw = "still reasoning</thinking>Corrected."
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_raises_when_content_is_null():
    backend = OllamaBackend(http_post=lambda url, payload: {"choices": [{"message": {"content": None}}]})
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_raises_when_message_is_null():
    backend = OllamaBackend(http_post=lambda url, payload: {"choices": [{"message": None}]})
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_raises_when_first_choice_is_null():
    backend = OllamaBackend(http_post=lambda url, payload: {"choices": [None]})
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_cli_backend_invokes_claude_p_with_the_prompt():
    calls = []

    def runner(argv, input_text):
        calls.append((argv, input_text))
        return "Corrected.\n"

    assert ClaudeCLIBackend(runner=runner).transform("corected") == "Corrected."
    argv, input_text = calls[0]
    assert argv[0] == "claude"
    assert "-p" in argv
    assert "corected" in input_text


def test_cli_backend_raises_when_the_cli_returns_nothing():
    with pytest.raises(BackendRefusal):
        ClaudeCLIBackend(runner=lambda argv, input_text: "   ").transform("x")


def test_cli_backend_raises_when_subprocess_times_out(monkeypatch):
    def mock_run(*args, **kwargs):
        raise subprocess.TimeoutExpired(cmd="claude", timeout=120)

    monkeypatch.setattr(subprocess, "run", mock_run)
    with pytest.raises(BackendRefusal, match="timeout"):
        _run_claude_cli(["claude", "-p"], "input")


def test_cli_backend_raises_when_claude_executable_not_found(monkeypatch):
    def mock_run(*args, **kwargs):
        raise FileNotFoundError("claude not found")

    monkeypatch.setattr(subprocess, "run", mock_run)
    with pytest.raises(BackendRefusal, match="executable not found"):
        _run_claude_cli(["claude", "-p"], "input")


from spike.backends import SYSTEM_PROMPT, AgentSDKBackend


def test_agent_sdk_backend_passes_prompt_and_text_to_run_query():
    calls = []

    def run_query(system_prompt, user_text):
        calls.append((system_prompt, user_text))
        return "Corrected.\n"

    assert AgentSDKBackend(run_query=run_query).transform("corected") == "Corrected."
    assert calls == [(SYSTEM_PROMPT, "corected")]


def test_agent_sdk_backend_raises_on_empty_output():
    with pytest.raises(BackendRefusal):
        AgentSDKBackend(run_query=lambda s, t: "").transform("x")


from spike.backends import AgentSDKSessionBackend


def test_agent_sdk_session_backend_passes_text_to_run_query():
    calls = []

    def run_query(user_text):
        calls.append(user_text)
        return "Corrected.\n"

    backend = AgentSDKSessionBackend(run_query=run_query)
    assert backend.transform("corected") == "Corrected."
    assert calls == ["corected"]


def test_agent_sdk_session_backend_raises_on_empty_output():
    with pytest.raises(BackendRefusal):
        AgentSDKSessionBackend(run_query=lambda t: "").transform("x")


def test_agent_sdk_session_backend_reuses_the_same_run_query_across_calls():
    """The point of this backend is a session held open across calls, so the
    adapter must invoke the same injected run_query for every transform()
    call rather than constructing a new one -- this is what the real
    implementation reuses to hold one ClaudeSDKClient open across calls."""
    calls = []

    def run_query(user_text):
        calls.append(user_text)
        return f"done:{user_text}"

    backend = AgentSDKSessionBackend(run_query=run_query)
    assert backend.transform("a") == "done:a"
    assert backend.transform("b") == "done:b"
    assert calls == ["a", "b"]


def test_agent_sdk_session_backend_name():
    assert AgentSDKSessionBackend(run_query=lambda t: "x").name == "agent-sdk-session"


import spike.backends as backends_module
from spike.backends import _AgentSDKSession


class _HangingClient:
    """Stands in for ClaudeSDKClient: query() returns immediately but
    receive_response() never yields a ResultMessage, mirroring the SDK's
    documented behaviour of iterating indefinitely on a hung session."""

    async def query(self, user_text):
        return None

    async def receive_response(self):
        await asyncio.Event().wait()  # never set; this await never returns
        yield  # pragma: no cover - unreachable, keeps this an async generator


def test_agent_sdk_session_raises_backend_refusal_when_it_never_completes(monkeypatch):
    # Use a short timeout so a correct fix returns almost instantly instead
    # of making this test wait the real 120s production timeout.
    monkeypatch.setattr(backends_module, "_SESSION_RESULT_TIMEOUT", 0.05)

    session = _AgentSDKSession.__new__(_AgentSDKSession)
    session._loop = asyncio.new_event_loop()
    session._thread = threading.Thread(target=session._loop.run_forever, daemon=True)
    session._thread.start()
    session._client = _HangingClient()

    outcome = {}

    def call_ask():
        try:
            outcome["result"] = session.ask("x")
        except BaseException as e:  # noqa: BLE001 - capturing across threads
            outcome["exception"] = e

    caller = threading.Thread(target=call_ask, daemon=True)
    caller.start()
    # Safety net: if the fix is missing and .ask() truly hangs forever, this
    # join times out and the test fails on the assertion below rather than
    # hanging the suite.
    caller.join(timeout=5)

    try:
        assert not caller.is_alive(), (
            "session.ask() did not return within the test's safety timeout; "
            "it appears to hang instead of raising BackendRefusal"
        )
        assert isinstance(outcome.get("exception"), BackendRefusal)
    finally:
        # The timed-out .ask() call leaves its underlying task parked forever
        # on session._loop (still awaiting the hanging client). Cancel it
        # explicitly before stopping the loop so it doesn't get garbage
        # collected mid-flight and print an unrelated "Task was destroyed
        # but it is pending!" line to stderr.
        async def _cancel_pending():
            current = asyncio.current_task()
            for task in asyncio.all_tasks():
                if task is not current:
                    task.cancel()
            await asyncio.sleep(0)

        try:
            asyncio.run_coroutine_threadsafe(_cancel_pending(), session._loop).result(timeout=1)
        except Exception:
            pass
        session._loop.call_soon_threadsafe(session._loop.stop)
        session._thread.join(timeout=1)
