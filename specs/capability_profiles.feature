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

  Scenario: Profile denies opening for the wrong principal
    Given a fresh resource registry
    And resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write,inspect"
    And capability profile "readonly" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "resource/open,resource/read"
    When holder "agent.beta" opens resource "markodb:papers" under the capability profile with grants "read"
    Then the resource denial reason is "capability_denied"
    And the resource denial missing capability is "profile/principal"
    And the resource audit decision is "denied"

  Scenario: Profile denies opening outside its trust zone
    Given a fresh resource registry
    And resource "secrets:vault" of type "secret.store" exists in trust zone "project.secrets" with operations "read"
    And capability profile "readonly" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "resource/open,resource/read"
    When holder "agent.alpha" opens resource "secrets:vault" under the capability profile with grants "read"
    Then the resource denial reason is "wrong_trust_zone"
    And the resource audit decision is "denied"

  Scenario: Profile allows domain-specific resource capabilities
    Given a fresh resource registry
    And resource "markodb:qbbn" of type "markodb.qbbn" exists in trust zone "project.markodb" with operation "ask" requiring capability "qbbn/ask"
    And resource adapter "qbbn.adapter" handles type "markodb.qbbn" with operations "ask"
    And capability profile "qbbn" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "resource/open,qbbn/ask"
    When holder "agent.alpha" opens resource "markodb:qbbn" under the capability profile with grants "qbbn/ask"
    And the holder executes resource operation "ask" through the adapter under the capability profile returning "entailed"
    Then the resource adapter call count is 1
    And the resource adapter string value is "entailed"
    And the resource operation audit decision is "allowed"

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
