Feature: Anvil planning scaffold
  Agents need executable acceptance specs that describe externally visible
  language and runtime behavior.

  Scenario: Project shape can be queried
    Given a fresh Anvil planning scaffold
    When the agent asks for the project shape
    Then the response says Anvil is in phase 0 planning
