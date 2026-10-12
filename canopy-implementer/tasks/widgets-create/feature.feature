Feature: Create a widget
  Scenario: A valid name creates a widget with a generated id
    When a client posts a widget named "Alpha"
    Then the response status is 201
    And the response has a generated id that is a UUID and the name "Alpha"

  Scenario: Two widgets created with the same name get different ids
    When a client posts a widget named "Alpha" twice
    Then the two responses have different ids
