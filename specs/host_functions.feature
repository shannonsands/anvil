Feature: Host functions
  Agents need embedded runtimes to expose Rust host functions as explicit,
  capability-checked imports without making untrusted language code part of the
  host process authority by default.

  Scenario: VM sessions call registered host functions
    Given a fresh VM session
    And host function "host/add" is registered
    When the VM session evaluates "(host/add 40 2)"
    Then the VM value prints as "42"
    And the host function call count is 1

  Scenario: Required modules call registered host functions
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
    And filesystem package source "src/planner/host_math.anv" contains "(define answer (host/add 40 2))"
    When the filesystem package module session is loaded
    Given host function "host/add" is registered
    When the module session evaluates "(require planner.host_math) answer"
    Then the VM value prints as "42"
    And the host function call count is 1

  Scenario: Capability denials stop host functions before invocation
    Given a fresh VM session
    And host function "host/secret" requiring capability "host/secret" in trust zone "project.markodb" is registered
    And the VM session uses capability profile "readonly" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "host/read"
    When the VM session evaluates "(host/secret)"
    Then the VM diagnostic code is "ANVIL_RUNTIME_HOST_CAPABILITY_DENIED"
    And the VM diagnostic phase is "runtime"
    And the host function call count is 0

  Scenario: Authorized profiles can call capability-guarded host functions
    Given a fresh VM session
    And host function "host/secret" requiring capability "host/secret" in trust zone "project.markodb" is registered
    And the VM session uses capability profile "dev" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "host/secret"
    When the VM session evaluates "(host/secret)"
    Then the VM value prints as ":authorized"
    And the host function call count is 1

  Scenario: Host failures surface as VM runtime diagnostics
    Given a fresh VM session
    And failing host function "host/flaky" is registered with message "backend timeout"
    When the VM session evaluates "(host/flaky)"
    Then the VM diagnostic code is "ANVIL_RUNTIME_HOST_CALL_FAILED"
    And the VM diagnostic phase is "runtime"
    And the host function call count is 1
