Feature: Anvil manifest parsing
  Agents need Anvil.toml to be inspectable and deterministic before package
  roots can feed module resolution.

  Scenario: Parse a minimal package manifest
    Given the manifest input
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    When the manifest is parsed
    Then the manifest package name is "planner-tools"
    And the manifest package version is "0.1.0"
    And the manifest lib module is "planner.tools"
    And the manifest lib path is "src/lib.anv"
    And the manifest source roots are "src"
    And the manifest test roots are "tests"
    And the manifest eval roots are "evals"
    And the manifest example roots are "examples"

  Scenario: Parse workspace and source roots
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
      tests = ["tests"]
      evals = ["evals"]
      examples = ["examples"]

      [workspace]
      members = ["packages/*", "tools/*"]
      """
    When the manifest is parsed
    Then the manifest source roots are "src,agents"
    And the manifest test roots are "tests"
    And the manifest eval roots are "evals"
    And the manifest example roots are "examples"
    And the manifest workspace members are "packages/*,tools/*"

  Scenario: Report missing package table
    Given the manifest input
      """
      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    When the manifest is parsed
    Then the manifest diagnostic code is "ANVIL_MANIFEST_MISSING_FIELD"
    And the manifest diagnostic phase is "manifest"

  Scenario: Report malformed manifest TOML
    Given the manifest input
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib
      module = "planner.tools"
      path = "src/lib.anv"
      """
    When the manifest is parsed
    Then the manifest diagnostic code is "ANVIL_MANIFEST_PARSE"
    And the manifest diagnostic phase is "manifest"
