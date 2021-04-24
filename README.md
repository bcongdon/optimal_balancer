# optimal_balancer

A simple tool for calculating the optimal number of shares to buy to maintain a proportional portfolio.

## Usage

```
optimal_balancer 1.0

USAGE:
    optimal_balancer [FLAGS] [OPTIONS] --config <config>

FLAGS:
    -d, --download-current-prices
    -h, --help                       Prints help information
    -V, --version                    Prints version information

OPTIONS:
    -c, --config <config>
    -t, --target-buy <target-buy>
```

### Examples

```sh
$ optimal_balancer --config path/to/config

# Download the current price for ticker symbols at runtime.
$ optimal_balancer --config path/to/config --download-current-prices
```

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
