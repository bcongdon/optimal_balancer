use anyhow::{anyhow, bail, Result};
use clap::{AppSettings, Clap};
use serde::Deserialize;
use z3::ast::{self, Real};
use z3::Context;

#[derive(Clap)]
#[clap(version = "1.0")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(short, long, default_value = "src/example.toml")]
    config: String,
}

#[derive(Deserialize)]
struct Fund {
    shares: f64,
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
        Ok(())
    }
}

fn f64_to_real(ctx: &Context, val: f64) -> Real {
    // NOTE: This is lossy, since we only use 3 decimal digits.
    ast::Real::from_real_str(ctx, &format!("{:.3}", val), "1").unwrap()
}

fn construct_model<'a>(
    ctx: &'a Context,
    funds: &Vec<Fund>,
    target_buy: f64,
) -> Option<z3::Model<'a>> {
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
    optimize.get_model()
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    let config_str = std::fs::read_to_string(opts.config)?;
    let config: Config = toml::from_str(&config_str)?;
    config.validate()?;
    let funds = config.funds;

    let ctx = Context::new(&z3::Config::new());

    let model = construct_model(&ctx, &funds, config.target_buy)
        .ok_or(anyhow!("evaluating model failed"))?;

    println!("Optimal purchasing strategy:");
    let mut total = 0.0;
    for f in funds {
        let shares = model
            .eval(&ast::Int::new_const(&ctx, f.symbol.clone()))
            .and_then(|s| s.as_i64())
            .ok_or(anyhow!("failed to evaluate {}", f.symbol))?;
        let purchase = f.price * (shares as f64);
        total += purchase;
        println!("{}:\t{} shares\t${:.2}", f.symbol, shares, purchase,);
    }
    println!("\nTotal purchase:\t\t${:.2}", total);

    Ok(())
}
