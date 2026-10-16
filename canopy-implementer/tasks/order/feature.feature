Feature: Order management

  Scenario: Place an order for one product
    Given a product "PROD-A" priced at 100.00
    When a customer "CUST-001" places an order for 1 of "PROD-A"
    Then the order total is 100.00

  Scenario Outline: A net total of 500.00 or more gets a 10 percent volume discount
    Given a product "PROD-B" priced at 60.00
    When a customer "CUST-002" places an order for <quantity> of "PROD-B"
    Then the order total is <total>

    Examples:
      | quantity | total  |
      | 8        | 480.00 |
      | 9        | 486.00 |
      | 10       | 540.00 |

  Scenario Outline: A quantity that is not positive is rejected
    Given a product "PROD-C" priced at 25.00
    When a customer "CUST-003" places an order for <quantity> of "PROD-C"
    Then the order is rejected with the message "Quantity must be positive"

    Examples:
      | quantity |
      | 0        |
      | -1       |

  Scenario: A placed order can be looked up by its id
    Given a product "PROD-A" priced at 100.00
    And a customer "CUST-001" has placed an order for 1 of "PROD-A"
    When the order is looked up by its id
    Then the order belongs to customer "CUST-001" and has the total 100.00
