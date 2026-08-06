import pytest

from spike.backends import SYSTEM_PROMPT, BackendRefusal, OllamaBackend


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


def test_raises_when_content_is_null():
    backend = OllamaBackend(http_post=lambda url, payload: {"choices": [{"message": {"content": None}}]})
    with pytest.raises(BackendRefusal):
        backend.transform("x")
