#[macro_use]
extern crate prettytable;

use anyhow::{anyhow, bail, Result};
use clap::{AppSettings, Clap};
use prettytable::Table;
use serde::Deserialize;
use yahoo_finance::history;
use z3::ast::{self, Real};
use z3::Context;

#[derive(Clap)]
#[clap(version = "1.0")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(short, long)]
    config: String,
    #[clap(short, long)]
    download_current_prices: bool,
    #[clap(short, long)]
    target_buy: Option<f64>,
}

#[derive(Deserialize)]
struct Fund {
    shares: f64,
    #[serde(default)]
    price: f64,
    symbol: String,
    target_proportion: f64,
}

#[derive(Deserialize)]
struct Config {
    target_buy: f64,
    funds: Vec<Fund>,
}

impl Config {
    fn validate(&self) -> Result<()> {
        let fund_proportion_sum: f64 = self.funds.iter().map(|f| f.target_proportion).sum();
        if (fund_proportion_sum - 1.0).abs() > 0.01 {
            bail!(
                "expected target_proportions to sum to 1.00, got {:}",
                fund_proportion_sum
            );
        }
        for f in self.funds.iter() {
            if f.price.is_sign_negative() || f.price == 0f64 {
                bail!("price for {} is not positive", f.symbol);
            }
        }
        Ok(())
    }
}

fn f64_to_real(ctx: &Context, val: f64) -> Real {
    // NOTE: This is lossy, since we only use 3 decimal digits.
    ast::Real::from_real_str(ctx, &format!("{:.3}", val), "1").unwrap()
}

fn construct_model<'a>(ctx: &'a Context, funds: &Vec<Fund>, target_buy: f64) -> Option<Model<'a>> {
    let optimize = z3::Optimize::new(&ctx);

    let mut vars = Vec::new();
    let mut total_bought = ast::Real::from_real(&ctx, 0, 1);
    let mut total_existing = ast::Real::from_real(&ctx, 0, 1);
    for f in funds.iter() {
        let v = ast::Int::new_const(&ctx, f.symbol.clone());
        optimize.assert(&v.ge(&ast::Int::from_i64(&ctx, 0)));
        let price = f64_to_real(&ctx, f.price);
        total_bought += ast::Real::from_int(&v) * &price;
        total_existing += f64_to_real(&ctx, f.shares) * &price;
        vars.push((f, v));
    }
    let new_total = &total_bought + &total_existing;

    let mut objective = ast::Real::from_real(&ctx, 0, 1);
    for f in funds.iter() {
        let v = ast::Int::new_const(&ctx, f.symbol.clone());
        let price = f64_to_real(&ctx, f.price);
        let delta_from_ideal = (price * (ast::Real::from_int(&v) + f64_to_real(&ctx, f.shares)))
            - (&new_total * &f64_to_real(&ctx, f.target_proportion));
        objective += delta_from_ideal
            .clone()
            .lt(&ast::Real::from_real(&ctx, 0, 1))
            .ite(&(-delta_from_ideal.clone()), &delta_from_ideal.clone());
    }

    let target_buy = &f64_to_real(&ctx, target_buy);
    optimize.assert(&total_bought.lt(&target_buy));

    // Add penalty for going below the target amount
    objective += (target_buy - total_bought) * f64_to_real(&ctx, 1.0);
    optimize.minimize(&objective);

    optimize.check(&[]);
    optimize.get_model().map(|model| Model {
        ctx,
        model,
        new_total,
    })
}

struct Model<'a> {
    ctx: &'a z3::Context,
    model: z3::Model<'a>,
    new_total: z3::ast::Real<'a>,
}

impl<'a> Model<'a> {
    fn optimal_shares(&self, fund: &Fund) -> Option<i64> {
        self.model
            .eval(&ast::Int::new_const(self.ctx, fund.symbol.clone()))
            .and_then(|s| s.as_i64())
    }

    fn new_proportion(&self, fund: &Fund) -> Option<f64> {
        match self.optimal_shares(&fund) {
            Some(shares) => self
                .new_portfolio_total()
                .map(|total| ((shares as f64) + fund.shares) * fund.price / total),
            None => None,
        }
    }

    fn new_portfolio_total(&self) -> Option<f64> {
        self.model
            .eval(&self.new_total)
            .and_then(|total| total.as_real())
            .map(|(num, dem)| (num as f64) / (dem as f64))
    }
}

async fn fund_price(symbol: &str) -> Result<f64> {
    let history = history::retrieve_interval(symbol, yahoo_finance::Interval::_1d).await?;
    match history.first() {
        Some(bar) => Ok(bar.close),
        None => bail!("empty history returned for {}", symbol),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    let config_str = std::fs::read_to_string(opts.config)?;
    let mut config: Config = toml::from_str(&config_str)?;

    if opts.download_current_prices {
        println!("Downloading current fund prices...\nCurrent prices:");
        for f in config.funds.iter_mut() {
            f.price = fund_price(&f.symbol).await?;
            println!("{}:\t${:.2}", f.symbol, f.price);
        }
        println!("");
    }

    config.validate()?;
    let funds = config.funds;

    let target_buy = match opts.target_buy {
        Some(val) => val,
        None => config.target_buy,
    };

    let ctx = Context::new(&z3::Config::new());
    let model =
        construct_model(&ctx, &funds, target_buy).ok_or(anyhow!("evaluating model failed"))?;

    println!("Optimal purchasing strategy:");
    let mut table = Table::new();
    table.add_row(row![b->"Fund", b->"Shares to Buy", b->"Buy Amt", b->"New Proportion"]);
    let mut total = 0.0;
    for f in funds {
        let shares = model
            .optimal_shares(&f)
            .ok_or(anyhow!("failed to evaluate {}", f.symbol))?;
        let purchase = f.price * (shares as f64);
        total += purchase;
        let new_proportion = model
            .new_proportion(&f)
            .ok_or(anyhow!("unable to get new proportion for {}", f.symbol))?;
        table.add_row(row![
            bc->f.symbol,
            r->shares,
            r->format!("${:.2}", purchase),
            r->format!("{:.2}%", new_proportion * 100.0),
        ]);
    }
    table.printstd();
    println!("\nTotal purchase:\t\t${:.2}", total);
    println!(
        "New portfolio total: \t${:.2}",
        model.new_portfolio_total().unwrap()
    );

    Ok(())
}
