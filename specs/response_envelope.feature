Feature: Response envelope
  Agents need one stable eval response shape that stays concise by default
  while still allowing debug facets when requested.

  Scenario: Serialize successful VM evaluation as a concise response envelope
    Given a fresh VM session
    When the VM session evaluates "(+ 40 2)" as a response envelope
    Then the response envelope status is "ok"
    And the response envelope kind is "eval_result"
    And the response envelope summary is "42"
    And the response envelope value display is "42"
    And the response envelope value kind is "integer"
    And the response envelope metadata includes VM execution metrics
    And the response envelope omits debug facets

  Scenario: Serialize runtime diagnostics as a response envelope
    Given a fresh VM session
    When the VM session evaluates "missing" as a response envelope
    Then the response envelope status is "error"
    And the response envelope kind is "eval_result"
    And the response envelope diagnostic code is "ANVIL_RUNTIME_UNBOUND_SYMBOL"
    And the response envelope diagnostic phase is "runtime"
    And the response envelope diagnostic primary span starts at line 1 column 1

  Scenario: Debug response envelopes include opt-in facets
    Given a fresh VM session
    When the VM session evaluates "(+ 1 2)" as a debug response envelope
    Then the response envelope status is "ok"
    And the response envelope has facet "vm.metrics"
