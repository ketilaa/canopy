Feature: Create a widget
  Scenario: A valid name creates a widget with a generated id
    When a client posts a widget named "Alpha"
    Then the response status is 201
    And the response has a generated id and the name "Alpha"

  Scenario Outline: An invalid name is rejected
    When a client posts a widget with <name>
    Then the response status is 400

    Examples:
      | name                    |
      | a blank name            |
      | no name                 |
      | a name of 51 characters |
