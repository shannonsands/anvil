Feature: Workspace loading
  Agents need Cargo-shaped workspace members to feed deterministic module
  resolution before lockfiles and dependency registries exist.

  Scenario: Load a workspace member into the resolver
    Given a filesystem package with manifest
      """
      [package]
      name = "root-tools"
      version = "0.1.0"

      [lib]
      module = "root.tools"
      path = "src/lib.anv"

      [workspace]
      members = ["packages/*"]
      """
    And filesystem package source "src/lib.anv" contains "(define root true)"
    And filesystem package file "packages/planner/Anvil.toml"
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "packages/planner/src/lib.anv" contains "(define planner true)"
    And filesystem package source "packages/planner/src/planner/search.anv" contains "(define search true)"
    When the filesystem workspace snapshot is loaded
    And the module resolver resolves "planner.search"
    Then the module resolution root kind is "workspace"
    And the module resolution root name is "planner-tools"
    And the module resolution path is "packages/planner/src/planner/search.anv"

  Scenario: Prefer the root package over a workspace member
    Given a filesystem package with manifest
      """
      [package]
      name = "root-tools"
      version = "0.1.0"

      [lib]
      module = "root.tools"
      path = "src/lib.anv"

      [workspace]
      members = ["packages/*"]
      """
    And filesystem package source "src/lib.anv" contains "(define root true)"
    And filesystem package source "src/planner/search.anv" contains "(define root-search true)"
    And filesystem package file "packages/planner/Anvil.toml"
      """
      [package]
      name = "planner-tools"
      version = "0.1.0"

      [lib]
      module = "planner.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "packages/planner/src/lib.anv" contains "(define planner true)"
    And filesystem package source "packages/planner/src/planner/search.anv" contains "(define search true)"
    When the filesystem workspace snapshot is loaded
    And the module resolver resolves "planner.search"
    Then the module resolution root kind is "package"
    And the module resolution root name is "root-tools"
    And the module resolution path is "src/planner/search.anv"

  Scenario: Report a missing workspace member manifest
    Given a filesystem package with manifest
      """
      [package]
      name = "root-tools"
      version = "0.1.0"

      [lib]
      module = "root.tools"
      path = "src/lib.anv"

      [workspace]
      members = ["packages/*"]
      """
    And filesystem package source "src/lib.anv" contains "(define root true)"
    And filesystem package source "packages/broken/src/lib.anv" contains "(define broken true)"
    When the filesystem workspace snapshot is loaded
    Then the project diagnostic code is "ANVIL_PROJECT_WORKSPACE_MEMBER_MANIFEST_NOT_FOUND"
    And the project diagnostic phase is "project"

  Scenario: Report ambiguous workspace modules
    Given a filesystem package with manifest
      """
      [package]
      name = "root-tools"
      version = "0.1.0"

      [lib]
      module = "root.tools"
      path = "src/lib.anv"

      [workspace]
      members = ["packages/*"]
      """
    And filesystem package source "src/lib.anv" contains "(define root true)"
    And filesystem package file "packages/alpha/Anvil.toml"
      """
      [package]
      name = "alpha-tools"
      version = "0.1.0"

      [lib]
      module = "alpha.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "packages/alpha/src/lib.anv" contains "(define alpha true)"
    And filesystem package source "packages/alpha/src/planner/search.anv" contains "(define alpha-search true)"
    And filesystem package file "packages/beta/Anvil.toml"
      """
      [package]
      name = "beta-tools"
      version = "0.1.0"

      [lib]
      module = "beta.tools"
      path = "src/lib.anv"
      """
    And filesystem package source "packages/beta/src/lib.anv" contains "(define beta true)"
    And filesystem package source "packages/beta/src/planner/search.anv" contains "(define beta-search true)"
    When the filesystem workspace snapshot is loaded
    And the module resolver resolves "planner.search"
    Then the module diagnostic code is "ANVIL_MODULE_AMBIGUOUS"
    And the module diagnostic phase is "module"
