Feature: Resource handles
  Agents need resource handles to be typed, inspectable, revocable, and checked
  at use time before host adapters can be safely exposed.

  Scenario: Open a typed resource handle
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "read,inspect"
    Then the resource handle type is "markodb.collection"
    And the resource handle grants include "read"
    And the resource handle display hides the raw token

  Scenario: Deny missing capabilities at use time
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "read"
    And the holder uses the resource handle for operation "write"
    Then the resource denial reason is "missing_capability"
    And the resource denial phase is "resource"
    And the resource audit decision is "denied"

  Scenario: Delegate a narrowed resource handle
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "read,write"
    And the holder delegates the resource handle to "actor.worker" with grants "read"
    Then the delegated resource handle holder is "actor.worker"
    And the delegated resource handle grants include "read"
    And the delegated resource handle grants do not include "write"

  Scenario: Reject delegation that widens authority
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "read"
    And the holder delegates the resource handle to "actor.worker" with grants "read,write"
    Then the resource denial reason is "delegation_denied"
    And the resource audit decision is "denied"

  Scenario: Revocation blocks future resource operations
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "read"
    And the supervisor revokes the resource handle
    And the holder uses the resource handle for operation "read"
    Then the resource denial reason is "handle_revoked"
    And the resource audit decision is "denied"
