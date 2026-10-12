Feature: Reject an invalid widget
  Scenario Outline: An invalid name is rejected
    When a client posts a widget with <name>
    Then the response status is 400

    Examples:
      | name                    |
      | a blank name            |
      | no name                 |
      | a name of 51 characters |

  Scenario: A name of exactly 50 characters is accepted
    When a client posts a widget with a name of 50 characters
    Then the response status is 201
