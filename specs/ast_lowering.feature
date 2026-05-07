Feature: Core AST lowering
  Agents need a source-aware AST surface before Anvil can compile to bytecode
  or expand macros.

  Scenario: Lower a definition form
    Given the agent input "(define answer (+ 40 2))"
    When the syntax layer lowers the input
    Then the AST contains one expression
    And the first AST kind is "define"
    And the first AST prints as "(define answer (+ 40 2))"

  Scenario: Lower an initial function form
    Given the agent input "(fn [x y] (+ x y))"
    When the syntax layer lowers the input
    And the AST response is serialized as JSON
    Then the JSON status is "ast"
    And the JSON first AST kind is "fn"

  Scenario: Report an invalid definition binding
    Given the agent input "(define 42 true)"
    When the syntax layer lowers the input
    Then the syntax diagnostic code is "ANVIL_SYNTAX_EXPECTED_SYMBOL"
    And the syntax diagnostic phase is "syntax"
