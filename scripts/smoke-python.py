"""Exercise the installed Python binding against the shared simulation fixtures."""

from __future__ import annotations

import json
from pathlib import Path

import axc_eval


ROOT = Path(__file__).resolve().parent.parent


def main() -> None:
    fixture = json.loads((ROOT / "fixtures" / "simulation.fixtures.json").read_text(encoding="utf-8"))
    cases = fixture["cases"]

    for case in cases:
        actual = axc_eval.simulate_condition(
            case["condition"],
            case["catalog"],
            case["record"],
            case["allowEmpty"],
        )
        if actual != case["expected"]:
            raise AssertionError(f"simulation output differs for {case['name']}")
        matched = axc_eval.matches(
            case["condition"],
            case["catalog"],
            case["record"],
            case["allowEmpty"],
        )
        if matched != case["expected"]["matched"]:
            raise AssertionError(f"match result differs for {case['name']}")

    evidence: list[dict[str, object]] = []
    first = cases[0]
    axc_eval.simulate_condition(
        first["condition"],
        first["catalog"],
        first["record"],
        first["allowEmpty"],
        evidence.append,
    )
    if len(evidence) != 1 or evidence[0]["input"]["operation"] != "simulate":
        raise AssertionError("simulation must emit exactly one explicit evidence snapshot")

    print(f"Python binding: {len(cases)} simulation fixtures passed")


if __name__ == "__main__":
    main()
