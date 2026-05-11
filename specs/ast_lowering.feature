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

  Scenario: Lower a lexical let form
    Given the agent input "(let [x 1 y (+ x 1)] y)"
    When the syntax layer lowers the input
    Then the AST contains one expression
    And the first AST kind is "let"
    And the first AST prints as "(let [x 1 y (+ x 1)] y)"

  Scenario: Report an invalid definition binding
    Given the agent input "(define 42 true)"
    When the syntax layer lowers the input
    Then the syntax diagnostic code is "ANVIL_SYNTAX_EXPECTED_SYMBOL"
    And the syntax diagnostic phase is "syntax"

  Scenario: Report invalid lexical let bindings
    Given the agent input "(let [x] x)"
    When the syntax layer lowers the input
    Then the syntax diagnostic code is "ANVIL_SYNTAX_BINDING_VECTOR"
    And the syntax diagnostic phase is "syntax"

  Scenario: Lower a require form
    Given the agent input "(require [planner.search :as search])"
    When the syntax layer lowers the input
    Then the AST contains one expression
    And the first AST kind is "require"
    And the first require import module is "planner.search"
    And the first require import alias is "search"
    And the first AST prints as "(require [planner.search :as search])"

  Scenario: Resolve require imports during AST lowering
    Given a fresh module resolver
    And module "planner.search" exists in package root "planner-tools" at "src/planner/search.anv"
    And the agent input "(require planner.search)"
    When the syntax layer lowers the input with the module resolver
    Then the AST contains one expression
    And the first AST kind is "require"
    And the first require import module is "planner.search"
    And the first require import resolution root kind is "package"
    And the first require import resolution path is "src/planner/search.anv"

  Scenario: Report require module diagnostics at the module source span
    Given a fresh module resolver
    And the agent input "(require missing.module)"
    When the syntax layer lowers the input with the module resolver
    Then the module diagnostic code is "ANVIL_MODULE_NOT_FOUND"
    And the module diagnostic phase is "module"
    And the module diagnostic primary span starts at line 1 column 10
