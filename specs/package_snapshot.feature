Feature: Manifest-backed package snapshots
  Agents need a deterministic bridge from Anvil.toml and known package files
  into module resolution before full filesystem project loading exists.

  Scenario: Register the package library module
    Given the manifest input
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    When the package snapshot builds a module resolver
    And the module resolver resolves "planner.tools"
    Then the module resolution root kind is "package"
    And the module resolution root name is "planner-tools"
    And the module resolution path is "src/lib.anv"

  Scenario: Derive package modules from source roots
    Given the manifest input
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"

      [source]
      roots = ["src", "agents"]
      """
    And package source "src/planner/search.anv" contains "(define search true)"
    And package source "agents/agent/tool.anv" contains "(define tool true)"
    When the package snapshot builds a module resolver
    And the module resolver resolves "planner.search"
    Then the module resolution root kind is "package"
    And the module resolution path is "src/planner/search.anv"
    When the module resolver resolves "agent.tool"
    Then the module resolution root kind is "package"
    And the module resolution path is "agents/agent/tool.anv"

  Scenario: Ignore package files outside source roots
    Given the manifest input
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    And package source "scratch/planner/search.anv" contains "(define ignored true)"
    When the package snapshot builds a module resolver
    And the module resolver resolves "planner.search"
    Then the module diagnostic code is "ANVIL_MODULE_NOT_FOUND"
    And the module diagnostic phase is "module"
