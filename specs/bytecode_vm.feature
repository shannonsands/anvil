Feature: Bytecode VM foundation
  Agents need a small executable VM surface before modules, closures, host
  calls, and debugger attachment become real runtime behavior.

  Scenario: Run a literal program
    Given the agent input "42"
    When the bytecode VM runs the input
    Then the VM value prints as "42"

  Scenario: Run top-level expressions and do forms
    Given the agent input
      """
      1
      (do 2 3)
      """
    When the bytecode VM runs the input
    Then the VM value prints as "3"

  Scenario: Run branch forms with Clojure-like falsey values
    Given the agent input "(if nil :yes :no)"
    When the bytecode VM runs the input
    Then the VM value prints as ":no"

  Scenario: Run vectors and ordered maps
    Given the agent input "(do [1 2] {:ok true :answer 42})"
    When the bytecode VM runs the input
    Then the VM value prints as "{:ok true :answer 42}"

  Scenario: Run top-level definitions and symbol lookup
    Given the agent input
      """
      (define answer (+ 40 2))
      answer
      """
    When the bytecode VM runs the input
    Then the VM value prints as "42"

  Scenario: Run bootstrap numeric primitives
    Given the agent input "(* (+ 2 4) 7)"
    When the bytecode VM runs the input
    Then the VM value prints as "42"

  Scenario: Report unsupported forms during compilation
    Given the agent input "(fn [x] x)"
    When the bytecode VM runs the input
    Then the VM diagnostic code is "ANVIL_COMPILE_UNSUPPORTED_FORM"
    And the VM diagnostic phase is "compile"
    And the VM diagnostic primary span starts at line 1 column 1

  Scenario: Report unbound symbols during runtime
    Given the agent input "missing"
    When the bytecode VM runs the input
    Then the VM diagnostic code is "ANVIL_RUNTIME_UNBOUND_SYMBOL"
    And the VM diagnostic phase is "runtime"
    And the VM diagnostic primary span starts at line 1 column 1

  Scenario: Report unsupported calls during compilation
    Given the agent input "(unknown 1)"
    When the bytecode VM runs the input
    Then the VM diagnostic code is "ANVIL_COMPILE_UNSUPPORTED_CALL"
    And the VM diagnostic phase is "compile"
    And the VM diagnostic primary span starts at line 1 column 1

  Scenario: Report runtime fuel exhaustion
    Given the agent input "42"
    When the bytecode VM runs the input with 0 instruction fuel
    Then the VM diagnostic code is "ANVIL_RUNTIME_FUEL_EXHAUSTED"
    And the VM diagnostic phase is "runtime"
    And the VM diagnostic primary span starts at line 1 column 1
