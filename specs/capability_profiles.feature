Feature: Capability profiles
  Agents need resource authority to be checked against process capability
  profiles before host adapters, delegation, or revocation can run.

  Scenario: Profile opens a read resource handle
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    And capability profile "readonly" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "resource/open,resource/read"
    When holder "agent.alpha" opens resource "markodb:papers" under the capability profile with grants "read"
    Then the resource handle grants include "read"

  Scenario: Profile denies opening an ungranted write handle
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    And capability profile "readonly" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "resource/open,resource/read"
    When holder "agent.alpha" opens resource "markodb:papers" under the capability profile with grants "write"
    Then the resource denial reason is "capability_denied"
    And the resource denial missing capability is "resource/write"
    And the resource audit decision is "denied"

  Scenario: Profile denial stops adapter execution
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    And resource adapter "markodb.adapter" handles type "markodb.collection" with operations "write"
    And capability profile "readonly" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "resource/open,resource/read"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "write"
    And the holder executes resource operation "write" through the adapter under the capability profile returning "ignored"
    Then the resource denial reason is "capability_denied"
    And the resource denial missing capability is "resource/write"
    And the resource adapter call count is 0

  Scenario: Profile denies delegation without delegate authority
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    And capability profile "readonly" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "resource/open,resource/read"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "read"
    And the holder delegates the resource handle to "actor.worker" under the capability profile with grants "read"
    Then the resource denial reason is "capability_denied"
    And the resource denial missing capability is "resource/delegate"

  Scenario: Profile revokes a resource handle
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    And capability profile "security" for principal "agent.security" in trust zone "project.markodb" with capabilities "resource/revoke"
    When holder "agent.alpha" opens resource "markodb:papers" with grants "read"
    And the capability profile revokes the resource handle
    And the holder uses the resource handle for operation "read"
    Then the resource denial reason is "handle_revoked"
    And the resource audit decision is "denied"
