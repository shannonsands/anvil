Feature: Filesystem package loading
  Agents need Anvil.toml plus real package files to become a deterministic
  package snapshot before broader workspace and lockfile loading exists.

  Scenario: Load a filesystem package into the module resolver
    Given a filesystem package with manifest
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
    And filesystem package source "src/lib.anv" contains "(define answer 42)"
    And filesystem package source "src/planner/search.anv" contains "(define search true)"
    And filesystem package source "agents/agent/tool.anv" contains "(define tool true)"
    When the filesystem package snapshot is loaded
    And the module resolver resolves "planner.search"
    Then the module resolution root kind is "package"
    And the module resolution root name is "planner-tools"
    And the module resolution path is "src/planner/search.anv"
    When the module resolver resolves "agent.tool"
    Then the module resolution root kind is "package"
    And the module resolution path is "agents/agent/tool.anv"

  Scenario: Report a missing filesystem manifest
    Given an empty filesystem package
    When the filesystem package snapshot is loaded
    Then the project diagnostic code is "ANVIL_PROJECT_MANIFEST_NOT_FOUND"
    And the project diagnostic phase is "project"

  Scenario: Report a missing declared library source
    Given a filesystem package with manifest
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "src/other.anv" contains "(define other true)"
    When the filesystem package snapshot is loaded
    Then the project diagnostic code is "ANVIL_PROJECT_LIB_NOT_FOUND"
    And the project diagnostic phase is "project"

  Scenario: Report a missing declared source root
    Given a filesystem package with manifest
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"

      [source]
      roots = ["src"]
      """
    When the filesystem package snapshot is loaded
    Then the project diagnostic code is "ANVIL_PROJECT_SOURCE_ROOT_NOT_FOUND"
    And the project diagnostic phase is "project"
