Feature: Resource adapter execution
  Agents need host/resource adapters to run only after handle authorization and
  to report structured outcomes or adapter failures.

  Scenario: Execute an authorized resource operation through an adapter
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    And resource adapter "markodb.adapter" handles type "markodb.collection" with operations "read"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "read"
    And the holder executes resource operation "read" through the adapter returning "paper-count"
    Then the resource adapter call count is 1
    And the resource adapter output status is "completed"
    And the resource adapter string value is "paper-count"
    And the resource execution mode is "effectful"
    And the resource operation audit decision is "allowed"

  Scenario: Deny before an adapter call when authority is missing
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    And resource adapter "markodb.adapter" handles type "markodb.collection" with operations "write"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "read"
    And the holder executes resource operation "write" through the adapter returning "ignored"
    Then the resource denial reason is "missing_capability"
    And the resource adapter call count is 0

  Scenario: Convert adapter failures into resource diagnostics
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    And resource adapter "markodb.adapter" handles type "markodb.collection" with operations "read"
    And the resource adapter will fail with "backend timeout"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "read"
    And the holder executes resource operation "read" through the adapter returning "ignored"
    Then the resource denial reason is "adapter_failure"
    And the resource denial phase is "resource"
    And the resource adapter call count is 1
