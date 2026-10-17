Feature: Stock reservation

  Scenario: Received stock is available
    When 10 units of "SKU-1" are received
    Then 10 units of "SKU-1" are available

  Scenario: Receiving more stock adds to what is there
    Given 10 units of "SKU-1" are in stock
    When 5 more units are received
    Then 15 units are available

  Scenario: Reserving takes units out of what is available
    Given 10 units of "SKU-1" are in stock
    When 4 units are reserved
    Then 6 units are available

  Scenario: Reserving more than is available is rejected
    Given 10 units of "SKU-1" are in stock
    When 11 units are reserved
    Then the reservation is rejected with the message "Insufficient stock"
    And 10 units are still available

  Scenario: Reserving an unknown SKU is rejected
    When 1 unit of "SKU-9" is reserved
    Then the reservation is rejected with the message "Unknown SKU"

  Scenario Outline: A quantity that is not positive is rejected
    When <quantity> units of "SKU-1" are received or reserved
    Then the request is rejected with the message "Quantity must be positive"

    Examples:
      | quantity |
      | 0        |
      | -3       |
