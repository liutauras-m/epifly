use agent_core::{InvoiceData, InvoicePipeline, PlanTier, TenantContext};
use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "invoice-demo",
    about = "Extract structured data from an invoice image"
)]
struct Args {
    /// Path to the invoice image (PNG, JPEG)
    #[arg(default_value = "invoice.png")]
    image: PathBuf,

    /// Claude model to use
    #[arg(long, default_value = "claude-opus-4-7")]
    model: String,

    /// Tenant ID (simulates multitenant isolation)
    #[arg(long, default_value = "conusai-demo")]
    tenant_id: String,

    /// Plan tier: free | pro | enterprise
    #[arg(long, default_value = "enterprise")]
    plan: String,

    /// Output raw JSON
    #[arg(long)]
    json: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let args = Args::parse();

    let path = if args.image.is_absolute() {
        args.image.clone()
    } else {
        std::env::current_dir()?.join(&args.image)
    };

    anyhow::ensure!(path.exists(), "invoice image not found: {:?}", path);

    let plan = match args.plan.to_lowercase().as_str() {
        "free" => PlanTier::Free,
        "pro" => PlanTier::Pro,
        "enterprise" => PlanTier::Enterprise,
        other => anyhow::bail!("unknown plan: {other}"),
    };

    let tenant = TenantContext::new(
        &args.tenant_id,
        Some("user-demo".into()),
        plan,
        std::env::var("CONUSAI_WORKSPACE_ROOT")
            .unwrap_or_else(|_| "/tmp/conusai/workspaces".into()),
    );

    println!("{}", "═".repeat(60).cyan());
    println!("{}", "  ConusAI Invoice Extraction Pipeline".bold().cyan());
    println!("{}", "═".repeat(60).cyan());
    println!("  Tenant : {}", tenant.tenant_id.yellow());
    println!("  Plan   : {}", tenant.plan.to_string().yellow());
    println!("  Storage: {}", tenant.storage_prefix().dimmed());
    println!(
        "  Qdrant : {}",
        tenant.qdrant_collection("invoices").dimmed()
    );
    println!("  Image  : {}", path.display().to_string().yellow());
    println!("  Model  : {}", args.model.yellow());
    println!("{}", "─".repeat(60).cyan());
    println!("{}", "  Sending to Claude...".dimmed());

    let pipeline = InvoicePipeline::with_model(&args.model).with_tenant(tenant);

    let invoice = pipeline
        .extract_from_image_path(&path)
        .await
        .context("invoice extraction failed")?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&invoice)?);
        return Ok(());
    }

    print_invoice(&invoice);
    Ok(())
}

fn print_invoice(inv: &InvoiceData) {
    println!();
    println!("{}", "  INVOICE DETAILS".bold().green());
    println!("{}", "─".repeat(60).green());
    field("Invoice #", &inv.invoice_number);
    field("Date", &inv.invoice_date);
    field("Due Date", &inv.due_date);
    field("Order #", &inv.order_number);
    field("Status", &inv.status.as_ref().map(|s| s.to_uppercase()));

    println!();
    println!("{}", "  ISSUER".bold().blue());
    println!("{}", "─".repeat(60).blue());
    field("Name", &inv.issuer_name);
    field("Address", &inv.issuer_address);
    field("VAT", &inv.issuer_vat);

    println!();
    println!("{}", "  BILLED TO".bold().blue());
    println!("{}", "─".repeat(60).blue());
    field("Name", &inv.billed_to_name);
    field("Company", &inv.billed_to_company);
    field("Address", &inv.billed_to_address);
    field("Email", &inv.billed_to_email);

    if !inv.line_items.is_empty() {
        println!();
        println!("{}", "  LINE ITEMS".bold().magenta());
        println!("{}", "─".repeat(60).magenta());
        for item in &inv.line_items {
            let total = item
                .total
                .map(|t| format_money(t, inv.currency.as_deref()))
                .unwrap_or_else(|| "—".into());
            println!(
                "  {:.<45} {}",
                format!("{} ", item.description).dimmed(),
                total.bold()
            );
        }
    }

    println!();
    println!("{}", "  TOTALS".bold().yellow());
    println!("{}", "─".repeat(60).yellow());
    money_field("Subtotal", inv.subtotal, inv.currency.as_deref());
    money_field("Tax", inv.tax_amount, inv.currency.as_deref());
    money_field("Total", inv.total_amount, inv.currency.as_deref());
    money_field("Amount Due", inv.amount_due, inv.currency.as_deref());

    if let Some(notes) = &inv.notes {
        println!();
        println!("{}", "  NOTES".bold());
        println!("{}", "─".repeat(60));
        println!("  {}", notes.dimmed());
    }

    println!();
    println!("{}", "═".repeat(60).cyan());
    println!("{}", "  ✅ Extraction complete".bold().green());
    println!("{}", "═".repeat(60).cyan());
}

fn field(label: &str, val: &Option<impl std::fmt::Display>) {
    if let Some(v) = val {
        println!(
            "  {:15} {}",
            format!("{}:", label).dimmed(),
            v.to_string().bold()
        );
    }
}

fn money_field(label: &str, val: Option<f64>, currency: Option<&str>) {
    if let Some(v) = val {
        println!(
            "  {:15} {}",
            format!("{}:", label).dimmed(),
            format_money(v, currency).bold()
        );
    }
}

fn format_money(amount: f64, currency: Option<&str>) -> String {
    match currency.unwrap_or("EUR") {
        "EUR" => format!("€{:.2}", amount),
        "USD" => format!("${:.2}", amount),
        "GBP" => format!("£{:.2}", amount),
        c => format!("{} {:.2}", c, amount),
    }
}
