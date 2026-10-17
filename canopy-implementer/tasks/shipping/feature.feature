Feature: Parcel shipping cost

  Scenario Outline: The base price depends on the zone
    When a parcel of 1000 grams worth 50.00 is shipped to <zone>
    Then the shipping cost is <cost>

    Examples:
      | zone     | cost  |
      | DOMESTIC | 4.00  |
      | EUROPE   | 9.00  |
      | WORLD    | 15.00 |

  Scenario Outline: Each started 500 grams above 1000 grams adds 1.50
    When a parcel of <grams> grams worth 50.00 is shipped to DOMESTIC
    Then the shipping cost is <cost>

    Examples:
      | grams | cost  |
      | 1000  | 4.00  |
      | 1001  | 5.50  |
      | 1500  | 5.50  |
      | 1501  | 7.00  |
      | 30000 | 91.00 |

  Scenario Outline: A weight outside 1 to 30000 grams is rejected
    When a parcel of <grams> grams is shipped
    Then the shipment is rejected with the message "Weight must be between 1 and 30000 grams"

    Examples:
      | grams |
      | 0     |
      | -5    |
      | 30001 |

  Scenario: Domestic orders worth 100.00 or more ship free
    When a parcel of 2000 grams worth 100.00 is shipped to DOMESTIC
    Then the shipping cost is 0.00

  Scenario: Other zones never ship free
    When a parcel of 1000 grams worth 500.00 is shipped to EUROPE
    Then the shipping cost is 9.00
