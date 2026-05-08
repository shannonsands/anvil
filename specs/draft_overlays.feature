Feature: Draft overlays
  Agents need private draft overlays that behave like miniature worktrees
  before compile, test, approval, and activation workflows are implemented.

  Scenario: Create an in-memory draft module override
    Given a fresh draft overlay "session-1" owned by "agent.alpha"
    When the draft overlay adds module "planner.search" with source "(define answer 42)"
    Then the draft overlay status is "editing"
    And the draft overlay owner is "agent.alpha"
    And the first draft module name is "planner.search"
    And the first draft module source is "(define answer 42)"
    And the first draft module path is ".anvil/drafts/session-1/src/planner/search.anv"
    And the first draft module has 0 diagnostics
