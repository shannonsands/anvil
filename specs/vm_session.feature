Feature: VM-backed sessions
  Agents need a stateful evaluation surface so REPL interactions can build on
  previous definitions without losing source-aware diagnostics.

  Scenario: Preserve definitions across VM session evaluations
    Given a fresh VM session
    When the VM session evaluates "(define answer 42)"
    Then the VM value prints as "42"
    And the VM session binding "answer" prints as "42"
    When the VM session evaluates "answer"
    Then the VM value prints as "42"

  Scenario: Preserve quoted data across VM session evaluations
    Given a fresh VM session
    When the VM session evaluates "(define code '(+ 1 2))"
    Then the VM value prints as "(+ 1 2)"
    When the VM session evaluates "code"
    Then the VM value prints as "(+ 1 2)"

  Scenario: Preserve closures across VM session evaluations
    Given a fresh VM session
    When the VM session evaluates "(define make-adder (fn [x] (fn [y] (+ x y))))"
    Then the VM value prints as "#<fn:1>"
    When the VM session evaluates "(define add40 (make-adder 40))"
    Then the VM value prints as "#<fn:0>"
    When the VM session evaluates "(add40 2)"
    Then the VM value prints as "42"

  Scenario: Failed VM session evaluations do not corrupt previous state
    Given a fresh VM session
    When the VM session evaluates "(define answer 42)"
    Then the VM value prints as "42"
    When the VM session evaluates "missing"
    Then the VM diagnostic code is "ANVIL_RUNTIME_UNBOUND_SYMBOL"
    And the VM diagnostic phase is "runtime"
    When the VM session evaluates "answer"
    Then the VM value prints as "42"

  Scenario: Fuel exhaustion does not kill a VM session
    Given a fresh VM session
    When the VM session evaluates "(define answer 42)"
    Then the VM value prints as "42"
    Given the agent input "answer"
    When the VM session evaluates the input with 0 instruction fuel
    Then the VM diagnostic code is "ANVIL_RUNTIME_FUEL_EXHAUSTED"
    And the VM diagnostic phase is "runtime"
    When the VM session evaluates "answer"
    Then the VM value prints as "42"
