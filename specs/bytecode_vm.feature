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

  Scenario: Run named function values
    Given the agent input
      """
      (define add (fn [x y] (+ x y)))
      (add 40 2)
      """
    When the bytecode VM runs the input
    Then the VM value prints as "42"

  Scenario: Run direct function literal calls
    Given the agent input "((fn [x] (* x x)) 6)"
    When the bytecode VM runs the input
    Then the VM value prints as "36"

  Scenario: Run returned closures with captured lexical locals
    Given the agent input
      """
      (define make-adder (fn [x] (fn [y] (+ x y))))
      (define add40 (make-adder 40))
      (add40 2)
      """
    When the bytecode VM runs the input
    Then the VM value prints as "42"

  Scenario: Run lexical let bindings without leaking
    Given the agent input
      """
      (define x 10)
      (do
        (let [x 40 y (+ x 2)] y)
        x)
      """
    When the bytecode VM runs the input
    Then the VM value prints as "10"

  Scenario: Run closures captured from lexical let bindings
    Given the agent input
      """
      (define add40 (let [x 40] (fn [y] (+ x y))))
      (add40 2)
      """
    When the bytecode VM runs the input
    Then the VM value prints as "42"

  Scenario: Run tail-recursive functions with constant call depth
    Given the agent input
      """
      (define loop
        (fn [n acc]
          (if (= n 0)
            acc
            (loop (- n 1) (+ acc 1)))))
      (loop 1000 0)
      """
    When the bytecode VM runs the input with 50000 instruction fuel
    Then the VM value prints as "1000"
    And the VM max call depth is 2

  Scenario: Report unsupported forms during compilation
    Given the agent input "(require planner.search)"
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

  Scenario: Report non-callable values during runtime
    Given the agent input "(42)"
    When the bytecode VM runs the input
    Then the VM diagnostic code is "ANVIL_RUNTIME_NOT_CALLABLE"
    And the VM diagnostic phase is "runtime"
    And the VM diagnostic primary span starts at line 1 column 1

  Scenario: Report runtime fuel exhaustion
    Given the agent input "42"
    When the bytecode VM runs the input with 0 instruction fuel
    Then the VM diagnostic code is "ANVIL_RUNTIME_FUEL_EXHAUSTED"
    And the VM diagnostic phase is "runtime"
    And the VM diagnostic primary span starts at line 1 column 1
