Feature: Module execution
  Agents need require to load real package modules into stateful sessions before
  dynamic reload, bytecode caches, and staged replacement can be trusted.

  Scenario: Require a package module in a module session
    Given a filesystem package with manifest
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "src/lib.anv" contains "(define root true)"
    And filesystem package source "src/planner/search.anv" contains "(define add (fn [x y] (+ x y)))"
    When the filesystem package module session is loaded
    Given the agent input "(require planner.search) (add 40 2)"
    When the module session evaluates the input
    Then the VM value prints as "42"
    And the module session has loaded "package:planner-tools:planner.search"

  Scenario: Required module definitions remain available across session evaluations
    Given a filesystem package with manifest
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "src/lib.anv" contains "(define root true)"
    And filesystem package source "src/planner/search.anv" contains "(define answer 42)"
    When the filesystem package module session is loaded
    When the module session evaluates "(require planner.search)"
    Then the VM value prints as "nil"
    When the module session evaluates "answer"
    Then the VM value prints as "42"
    And the module session binding "answer" prints as "42"

  Scenario: Required modules can require dependencies
    Given a filesystem package with manifest
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "src/lib.anv" contains "(define root true)"
    And filesystem package source "src/planner/math.anv" contains "(define double (fn [x] (+ x x)))"
    And filesystem package source "src/planner/search.anv" contains "(require planner.math) (define answer (double 21))"
    When the filesystem package module session is loaded
    When the module session evaluates "(require planner.search) answer"
    Then the VM value prints as "42"
    And the module session has loaded "package:planner-tools:planner.math"
    And the module session has loaded "package:planner-tools:planner.search"

  Scenario: Failed required modules do not corrupt previous session state
    Given a filesystem package with manifest
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "src/lib.anv" contains "(define root true)"
    And filesystem package source "src/planner/bad.anv" contains "(define broken missing)"
    When the filesystem package module session is loaded
    When the module session evaluates "(define answer 42)"
    Then the VM value prints as "42"
    When the module session evaluates "(require planner.bad)"
    Then the VM diagnostic code is "ANVIL_RUNTIME_UNBOUND_SYMBOL"
    And the VM diagnostic phase is "runtime"
    When the module session evaluates "answer"
    Then the VM value prints as "42"

  Scenario: Module require cycles are diagnosed before evaluation loops forever
    Given a filesystem package with manifest
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "src/lib.anv" contains "(define root true)"
    And filesystem package source "src/planner/a.anv" contains "(require planner.b) (define a 1)"
    And filesystem package source "src/planner/b.anv" contains "(require planner.a) (define b 2)"
    When the filesystem package module session is loaded
    When the module session evaluates "(require planner.a)"
    Then the module diagnostic code is "ANVIL_MODULE_REQUIRE_CYCLE"
    And the module diagnostic phase is "module"

  Scenario: Module aliases are explicit future work
    Given a filesystem package with manifest
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "src/lib.anv" contains "(define root true)"
    And filesystem package source "src/planner/search.anv" contains "(define answer 42)"
    When the filesystem package module session is loaded
    When the module session evaluates "(require [planner.search :as search])"
    Then the module diagnostic code is "ANVIL_MODULE_ALIAS_UNSUPPORTED"
    And the module diagnostic phase is "module"
