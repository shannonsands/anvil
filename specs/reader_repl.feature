Feature: Reader-backed REPL
  Agents need an immediate REPL surface for testing syntax and diagnostics
  before Anvil can evaluate programs.

  Scenario: Parse a simple definition form
    Given the agent input "(define answer 42)"
    When the reader-backed REPL reads the input
    Then the response contains one datum
    And the first datum prints as "(define answer 42)"

  Scenario: Round-trip multiple top-level datums
    Given the agent input
      """
      (define answer 42)
      [answer {:ok true}]
      """
    When the reader-backed REPL reads the input
    Then the response contains 2 datums
    And the datums print as
      """
      (define answer 42)
      [answer {:ok true}]
      """

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

  Scenario: Evaluate across an interactive REPL session
    Given an empty REPL session
    When the REPL session reads the line "(define answer 42)"
    Then the REPL evaluation value prints as "42"
    When the REPL session reads the line "answer"
    Then the REPL evaluation value prints as "42"

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
    And the JSON diagnostic source id is "repl"
    And the JSON diagnostic severity is "error"
    And the JSON diagnostic phase is "reader"
    And the JSON diagnostic primary span starts at line 1 column 1
    And the JSON diagnostic has 1 suggestion

  Scenario: Render a source-aware reader diagnostic
    Given the agent input "(define answer 42"
    When the reader-backed REPL reads the input
    And the reader diagnostic is rendered as text
    Then the rendered diagnostic contains "--> repl:1:1"
    And the rendered diagnostic contains "1 | (define answer 42"
    And the rendered diagnostic contains "suggestion Add a matching )."
