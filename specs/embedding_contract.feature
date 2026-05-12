Feature: Host embedding contract
  Agents need embedded runtimes to expose one stable facade for eval,
  host functions, profiles, resources, budgets, and inspectable metadata.

  Scenario: Embedded runtime eval returns canonical response envelopes
    Given a fresh embedded runtime "agent-runtime"
    When the embedded runtime evaluates "(define answer 42)"
    Then the response envelope status is "ok"
    When the embedded runtime evaluates "answer"
    Then the response envelope status is "ok"
    And the response envelope value display is "42"
    When the embedded runtime facade is inspected
    Then the embedded runtime snapshot protocol is "anvil.embedding.v1"
    And the embedded runtime active profile is absent

  Scenario: Embedded runtime inspection reports host registrations
    Given a fresh embedded runtime "agent-runtime"
    And embedded host function "host/add" is registered
    When the embedded runtime facade is inspected
    Then the embedded runtime snapshot includes host function "host/add"
    And the embedded runtime host function "host/add" exact arity is 2

  Scenario: Embedded runtime active profile mediates host authority
    Given a fresh embedded runtime "agent-runtime"
    And embedded host function "host/secret" requiring capability "host/secret" in trust zone "project.markodb" is registered
    And embedded capability profile "readonly" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "host/read"
    When the embedded runtime activates profile "readonly"
    And the embedded runtime evaluates "(host/secret)"
    Then the response envelope status is "error"
    And the response envelope diagnostic code is "ANVIL_RUNTIME_HOST_CAPABILITY_DENIED"
    And the host function call count is 0

  Scenario: Embedded runtime opens resources under the active profile
    Given a fresh embedded runtime "agent-runtime"
    And embedded resource "markodb:papers" of type "markodb.collection" exists in trust zone "project.markodb" with operations "read,write"
    And embedded capability profile "readonly" for principal "agent.alpha" in trust zone "project.markodb" with capabilities "resource/open,resource/read"
    When the embedded runtime activates profile "readonly"
    And the embedded runtime opens resource "markodb:papers" with grants "read"
    Then the resource handle holder is "agent.alpha"
    And the resource handle grants include "read"
    When the embedded runtime facade is inspected
    Then the embedded runtime snapshot includes resource "markodb:papers"
    And the embedded runtime snapshot includes handle for resource "markodb:papers"
