Feature: Module resolution
  Agents need module resolution to be deterministic and inspectable before
  require, draft overlays, bytecode caches, and hot replacement can be trusted.

  Scenario: Resolve a package module before a workspace module
    Given a fresh module resolver
    And module "planner.search" exists in workspace root "workspace-tools" at "packages/tools/src/planner/search.anv"
    And module "planner.search" exists in package root "planner-tools" at "src/planner/search.anv"
    When the module resolver resolves "planner.search"
    Then the module resolution root kind is "package"
    And the module resolution root name is "planner-tools"
    And the module resolution path is "src/planner/search.anv"

  Scenario: Resolve a draft module before a workspace module
    Given a fresh module resolver
    And module "planner.search" exists in workspace root "workspace-tools" at "packages/tools/src/planner/search.anv"
    And module "planner.search" exists in draft root "session-1" at ".anvil/drafts/session-1/src/planner/search.anv"
    When the module resolver resolves "planner.search"
    Then the module resolution root kind is "draft"
    And the module resolution root name is "session-1"

  Scenario: Report ambiguous short module names
    Given a fresh module resolver
    And module "planner.search" exists in workspace root "planner-tools" at "src/planner/search.anv"
    And module "agent.search" exists in workspace root "agent-tools" at "src/agent/search.anv"
    When the module resolver resolves "search"
    Then the module diagnostic code is "ANVIL_MODULE_AMBIGUOUS"
    And the module diagnostic phase is "module"
    And the module diagnostic expected candidates include "planner.search"
    And the module diagnostic expected candidates include "agent.search"

  Scenario: Report missing modules
    Given a fresh module resolver
    When the module resolver resolves "missing.module"
    Then the module diagnostic code is "ANVIL_MODULE_NOT_FOUND"
    And the module diagnostic phase is "module"
