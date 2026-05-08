Feature: Syntax objects
  Agents need a span-preserving syntax-object surface before macros, modules,
  and expansion traces become real runtime behavior.

  Scenario: Wrap a reader datum as a syntax object
    Given the agent input "(define answer 42)"
    When the syntax object layer wraps the input
    Then the syntax object count is 1
    And the first syntax object id is "repl:1"
    And the first syntax object source id is "repl"
    And the first syntax object span starts at line 1 column 1
    And the first syntax object datum prints as "(define answer 42)"

  Scenario: Serialize the initial hygiene context
    Given the agent input "(fn [x] x)"
    When the syntax object layer wraps the input
    And the syntax object response is serialized as JSON
    Then the JSON status is "syntax"
    And the JSON first syntax context has 0 scopes
    And the JSON first syntax context has 0 marks

  Scenario: Preserve reader diagnostics through the syntax object layer
    Given the agent input "(define answer 42"
    When the syntax object layer wraps the input
    Then the syntax object diagnostic code is "ANVIL_READER_UNCLOSED_DELIMITER"
    And the syntax object diagnostic phase is "reader"
