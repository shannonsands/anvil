Feature: Reader-backed REPL
  Agents need an immediate REPL surface for testing syntax and diagnostics
  before Anvil can evaluate programs.

  Scenario: Parse a simple definition form
    Given the agent input "(define answer 42)"
    When the reader-backed REPL reads the input
    Then the response contains one datum
    And the first datum prints as "(define answer 42)"

  Scenario: Parse Clojure-like reader delimiters
    Given the agent input
      """
      [agent {:ok true :name "Anvil"} 'ready] ; ignored
      """
    When the reader-backed REPL reads the input
    Then the response contains one datum
    And the first datum prints as
      """
      [agent {:ok true :name "Anvil"} 'ready]
      """

  Scenario: Report an unclosed list with a structured diagnostic
    Given the agent input "(define answer 42"
    When the reader-backed REPL reads the input
    Then the response is a reader error
    And the diagnostic code is "ANVIL_READER_UNCLOSED_DELIMITER"

  Scenario: Continue reading an incomplete interactive form
    Given an empty REPL session
    When the REPL session reads the line "(define answer"
    Then the REPL session is waiting for more input
    When the REPL session reads the line "42)"
    Then the response contains one datum
    And the first datum prints as "(define answer 42)"

  Scenario: Serialize pending interactive input for agents
    Given an empty REPL session
    When the REPL session reads the line "(define answer"
    And the REPL interaction is serialized as JSON
    Then the JSON status is "pending"
    And the JSON diagnostic code is "ANVIL_READER_UNCLOSED_DELIMITER"
    And the JSON buffered line count is 1

  Scenario: Serialize reader diagnostics for agents
    Given the agent input "(define answer 42"
    When the reader-backed REPL reads the input
    And the REPL response is serialized as JSON
    Then the JSON status is "error"
    And the JSON diagnostic code is "ANVIL_READER_UNCLOSED_DELIMITER"
