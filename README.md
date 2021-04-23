# optimal_balancer

A simple tool for calculating the optimal number of shares to buy to maintain a proportional portfolio

## Usage

### Config File Format

```toml
# The desired total purchase price of new shares. ($)
target_buy = 6000.0

# A list of funds in the portfolio.
[[funds]]
symbol = "BND"              # The fund's ticker symbol.
shares = 100                # The number of shares already owned.
price = 85.40               # The current share price.
target_proportion = 0.15    # The desired fund allocation in the portfolio.

[[funds]]
symbol = "VTI"
shares =  200
price = 216.3
target_proportion = 0.70

[[funds]]
symbol = "VXUS"
shares =  100
price = 65.66
target_proportion = 0.15
```
