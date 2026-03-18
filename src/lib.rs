pub use reqwest;
pub use rust_decimal;
pub use solana_account_decoder;
pub use solana_sdk;

use anyhow::{Result, anyhow};
use reqwest::{Client, ClientBuilder};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use serde_json::Value;
use solana_account_decoder::UiAccount;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

pub mod field_as_string {
    use {
        serde::{Deserialize, Serialize},
        serde::{Deserializer, Serializer, de},
        std::str::FromStr,
    };

    pub fn serialize<T, S>(t: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: ToString,
        S: Serializer,
    {
        t.to_string().serialize(serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: FromStr,
        D: Deserializer<'de>,
        <T as FromStr>::Err: std::fmt::Debug,
    {
        let s: String = String::deserialize(deserializer)?;
        s.parse()
            .map_err(|e| de::Error::custom(format!("Parse error: {:?}", e)))
    }
}

pub mod option_field_as_string {
    use {
        serde::{Deserialize, Deserializer, Serialize, Serializer, de},
        std::str::FromStr,
    };

    pub fn serialize<T, S>(t: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: ToString,
        S: Serializer,
    {
        if let Some(t) = t {
            t.to_string().serialize(serializer)
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        T: FromStr,
        D: Deserializer<'de>,
        <T as FromStr>::Err: std::fmt::Debug,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        match opt {
            Some(s) => s
                .parse()
                .map(Some)
                .map_err(|e| de::Error::custom(format!("Parse error: {:?}", e))),
            None => Ok(None),
        }
    }
}

pub mod base64_serialize_deserialize {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde::{Deserializer, Serializer, de};

    use super::*;
    pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        let base58 = STANDARD.encode(v);
        String::serialize(&base58, s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let field_string = String::deserialize(deserializer)?;
        STANDARD
            .decode(field_string)
            .map_err(|e| de::Error::custom(format!("base64 decoding error: {:?}", e)))
    }
}

// ====================== ComputeUnitPrice & Priority ======================

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum ComputeUnitPriceMicroLamports {
    MicroLamports(u64),
    #[serde(deserialize_with = "deserialize_auto")]
    Auto,
}

fn deserialize_auto<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    enum Helper {
        #[serde(rename = "auto")]
        Variant,
    }
    Helper::deserialize(deserializer)?;
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PriorityLevel {
    Medium,
    High,
    VeryHigh,
}

#[derive(Deserialize, Debug, PartialEq, Copy, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub enum PrioritizationFeeLamports {
    AutoMultiplier(u32),
    JitoTipLamports(u64),
    #[serde(rename_all = "camelCase")]
    PriorityLevelWithMaxLamports {
        priority_level: PriorityLevel,
        max_lamports: u64,
        #[serde(default)]
        global: bool,
    },
    #[default]
    #[serde(untagged, deserialize_with = "deserialize_auto")]
    Auto,
    #[serde(untagged)]
    Lamports(u64),
    #[serde(untagged, deserialize_with = "deserialize_disabled")]
    Disabled,
}

fn deserialize_disabled<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    enum Helper {
        #[serde(rename = "disabled")]
        Variant,
    }
    Helper::deserialize(deserializer)?;
    Ok(())
}

impl Serialize for PrioritizationFeeLamports {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct AutoMultiplier {
            auto_multiplier: u32,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct JitoTip {
            jito_tip_lamports: u64,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct PriorityWrapper<'a> {
            priority_level_with_max_lamports: PriorityLevelWithMaxLamports<'a>,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct PriorityLevelWithMaxLamports<'a> {
            priority_level: &'a PriorityLevel,
            max_lamports: &'a u64,
            global: &'a bool,
        }

        match self {
            Self::AutoMultiplier(v) => AutoMultiplier {
                auto_multiplier: *v,
            }
            .serialize(serializer),
            Self::JitoTipLamports(v) => JitoTip {
                jito_tip_lamports: *v,
            }
            .serialize(serializer),
            Self::Auto => serializer.serialize_str("auto"),
            Self::Lamports(v) => serializer.serialize_u64(*v),
            Self::Disabled => serializer.serialize_str("disabled"),
            Self::PriorityLevelWithMaxLamports {
                priority_level,
                max_lamports,
                global,
            } => PriorityWrapper {
                priority_level_with_max_lamports: PriorityLevelWithMaxLamports {
                    priority_level,
                    max_lamports,
                    global,
                },
            }
            .serialize(serializer),
        }
    }
}

// ====================== TransactionConfig ======================

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DynamicSlippageSettings {
    pub min_bps: Option<u16>,
    pub max_bps: Option<u16>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct TransactionConfig {
    pub wrap_and_unwrap_sol: bool,
    pub allow_optimized_wrapped_sol_token_account: bool,
    #[serde(with = "option_field_as_string")]
    pub fee_account: Option<Pubkey>,
    #[serde(with = "option_field_as_string")]
    pub destination_token_account: Option<Pubkey>,
    #[serde(with = "option_field_as_string")]
    pub tracking_account: Option<Pubkey>,
    pub compute_unit_price_micro_lamports: Option<ComputeUnitPriceMicroLamports>,
    pub prioritization_fee_lamports: Option<PrioritizationFeeLamports>,
    pub dynamic_compute_unit_limit: bool,
    pub as_legacy_transaction: bool,
    pub use_shared_accounts: bool,
    pub use_token_ledger: bool,
    pub skip_user_accounts_rpc_calls: bool,
    pub keyed_ui_accounts: Option<Vec<KeyedUiAccount>>,
    pub program_authority_id: Option<u8>,
    pub dynamic_slippage: Option<DynamicSlippageSettings>,
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            wrap_and_unwrap_sol: true,
            allow_optimized_wrapped_sol_token_account: false,
            fee_account: None,
            destination_token_account: None,
            tracking_account: None,
            compute_unit_price_micro_lamports: None,
            prioritization_fee_lamports: Some(
                PrioritizationFeeLamports::PriorityLevelWithMaxLamports {
                    priority_level: PriorityLevel::VeryHigh,
                    max_lamports: 4_000_000,
                    global: false,
                },
            ),
            dynamic_compute_unit_limit: false,
            as_legacy_transaction: false,
            use_shared_accounts: true,
            use_token_ledger: false,
            skip_user_accounts_rpc_calls: false,
            keyed_ui_accounts: None,
            program_authority_id: None,
            dynamic_slippage: None,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct KeyedUiAccount {
    pub pubkey: String,
    #[serde(flatten)]
    pub ui_account: UiAccount,
    pub params: Option<Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SwapInfo {
    #[serde(with = "field_as_string")]
    pub amm_key: Pubkey,
    pub label: String,
    #[serde(with = "field_as_string")]
    pub input_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub output_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub in_amount: u64,
    #[serde(with = "field_as_string")]
    pub out_amount: u64,
    #[serde(default, with = "option_field_as_string")]
    pub fee_amount: Option<u64>,
    #[serde(default, with = "option_field_as_string")]
    pub fee_mint: Option<Pubkey>,
    // deprecated
    //
    // #[serde(default, with = "field_as_string")]
    // pub fee_amount: u64,
    // #[serde(default, with = "field_as_string")]
    // pub fee_mint: Pubkey,
}

pub type RoutePlanWithMetadata = Vec<RoutePlanStep>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlanStep {
    pub swap_info: SwapInfo,
    pub percent: u8,
}

#[derive(Serialize, Deserialize, Default, PartialEq, Clone, Debug)]
pub enum SwapMode {
    #[default]
    ExactIn,
    ExactOut,
}

impl FromStr for SwapMode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "ExactIn" => Ok(Self::ExactIn),
            "ExactOut" => Ok(Self::ExactOut),
            _ => Err(anyhow!("Invalid SwapMode: {}", s)),
        }
    }
}

#[derive(Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    #[serde(with = "field_as_string")]
    pub input_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub output_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub amount: u64,
    pub swap_mode: Option<SwapMode>,
    pub slippage_bps: u16,
    pub auto_slippage: Option<bool>,
    pub max_auto_slippage_bps: Option<u16>,
    pub compute_auto_slippage: bool,
    pub auto_slippage_collision_usd_value: Option<u32>,
    pub minimize_slippage: Option<bool>,
    pub platform_fee_bps: Option<u8>,
    pub dexes: Option<String>,
    pub excluded_dexes: Option<String>,
    pub only_direct_routes: Option<bool>,
    pub as_legacy_transaction: Option<bool>,
    pub restrict_intermediate_tokens: Option<bool>,
    pub max_accounts: Option<usize>,
    pub quote_type: Option<String>,
    pub quote_args: Option<HashMap<String, String>>,
    pub prefer_liquid_dexes: Option<bool>,
}

#[derive(Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InternalQuoteRequest {
    #[serde(with = "field_as_string")]
    pub input_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub output_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub amount: u64,
    pub swap_mode: Option<SwapMode>,
    pub slippage_bps: u16,
    pub auto_slippage: Option<bool>,
    pub max_auto_slippage_bps: Option<u16>,
    pub compute_auto_slippage: bool,
    pub auto_slippage_collision_usd_value: Option<u32>,
    pub minimize_slippage: Option<bool>,
    pub platform_fee_bps: Option<u8>,
    pub dexes: Option<String>,
    pub excluded_dexes: Option<String>,
    pub only_direct_routes: Option<bool>,
    pub as_legacy_transaction: Option<bool>,
    pub restrict_intermediate_tokens: Option<bool>,
    pub max_accounts: Option<usize>,
    pub quote_type: Option<String>,
    pub prefer_liquid_dexes: Option<bool>,
}

impl From<QuoteRequest> for InternalQuoteRequest {
    fn from(req: QuoteRequest) -> Self {
        Self {
            input_mint: req.input_mint,
            output_mint: req.output_mint,
            amount: req.amount,
            swap_mode: req.swap_mode,
            slippage_bps: req.slippage_bps,
            auto_slippage: req.auto_slippage,
            max_auto_slippage_bps: req.max_auto_slippage_bps,
            compute_auto_slippage: req.compute_auto_slippage,
            auto_slippage_collision_usd_value: req.auto_slippage_collision_usd_value,
            minimize_slippage: req.minimize_slippage,
            platform_fee_bps: req.platform_fee_bps,
            dexes: req.dexes,
            excluded_dexes: req.excluded_dexes,
            only_direct_routes: req.only_direct_routes,
            as_legacy_transaction: req.as_legacy_transaction,
            restrict_intermediate_tokens: req.restrict_intermediate_tokens,
            max_accounts: req.max_accounts,
            quote_type: req.quote_type,
            prefer_liquid_dexes: req.prefer_liquid_dexes,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlatformFee {
    #[serde(with = "field_as_string")]
    pub amount: u64,
    pub fee_bps: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    #[serde(with = "field_as_string")]
    pub input_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub in_amount: u64,
    #[serde(with = "field_as_string")]
    pub output_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub out_amount: u64,
    #[serde(with = "field_as_string")]
    pub other_amount_threshold: u64,
    pub swap_mode: SwapMode,
    pub slippage_bps: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_auto_slippage: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uses_quote_minimizing_slippage: Option<bool>,
    pub platform_fee: Option<PlatformFee>,
    pub price_impact_pct: Decimal,
    pub route_plan: RoutePlanWithMetadata,
    #[serde(default)]
    pub context_slot: u64,
    #[serde(default)]
    pub time_taken: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    #[serde(with = "field_as_string")]
    pub user_public_key: Pubkey,
    pub quote_response: QuoteResponse,
    #[serde(flatten)]
    pub config: TransactionConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PrioritizationType {
    #[serde(rename_all = "camelCase")]
    Jito { lamports: u64 },
    #[serde(rename_all = "camelCase")]
    ComputeBudget {
        micro_lamports: u64,
        estimated_micro_lamports: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DynamicSlippageReport {
    pub slippage_bps: u16,
    pub other_amount: Option<u64>,
    pub simulated_incurred_slippage_bps: Option<i16>,
    pub amplification_ratio: Option<Decimal>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UiSimulationError {
    error_code: String,
    error: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    #[serde(with = "base64_serialize_deserialize")]
    pub swap_transaction: Vec<u8>,
    pub last_valid_block_height: u64,
    pub prioritization_fee_lamports: u64,
    pub compute_unit_limit: u32,
    pub prioritization_type: Option<PrioritizationType>,
    pub dynamic_slippage_report: Option<DynamicSlippageReport>,
    pub simulation_error: Option<UiSimulationError>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SwapInstructionsResponse {
    pub token_ledger_instruction: Option<Instruction>,
    pub compute_budget_instructions: Vec<Instruction>,
    pub setup_instructions: Vec<Instruction>,
    pub swap_instruction: Instruction,
    pub cleanup_instruction: Option<Instruction>,
    pub other_instructions: Vec<Instruction>,
    pub address_lookup_table_addresses: Vec<Pubkey>,
    pub prioritization_fee_lamports: u64,
    pub compute_unit_limit: u32,
    pub prioritization_type: Option<PrioritizationType>,
    pub dynamic_slippage_report: Option<DynamicSlippageReport>,
    pub simulation_error: Option<UiSimulationError>,
}

// Internal structs for deserialization
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct InstructionInternal {
    #[serde(with = "field_as_string")]
    program_id: Pubkey,
    accounts: Vec<AccountMetaInternal>,
    #[serde(with = "base64_serialize_deserialize")]
    data: Vec<u8>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct AccountMetaInternal {
    #[serde(with = "field_as_string")]
    pubkey: Pubkey,
    is_signer: bool,
    is_writable: bool,
}

impl From<AccountMetaInternal> for AccountMeta {
    fn from(a: AccountMetaInternal) -> Self {
        AccountMeta {
            pubkey: a.pubkey,
            is_signer: a.is_signer,
            is_writable: a.is_writable,
        }
    }
}

impl From<InstructionInternal> for Instruction {
    fn from(i: InstructionInternal) -> Self {
        Instruction {
            program_id: i.program_id,
            accounts: i.accounts.into_iter().map(Into::into).collect(),
            data: i.data,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct SwapInstructionsResponseInternal {
    token_ledger_instruction: Option<InstructionInternal>,
    compute_budget_instructions: Vec<InstructionInternal>,
    setup_instructions: Vec<InstructionInternal>,
    swap_instruction: InstructionInternal,
    cleanup_instruction: Option<InstructionInternal>,
    other_instructions: Vec<InstructionInternal>,
    address_lookup_table_addresses: Vec<PubkeyInternal>,
    prioritization_fee_lamports: u64,
    compute_unit_limit: u32,
    prioritization_type: Option<PrioritizationType>,
    dynamic_slippage_report: Option<DynamicSlippageReport>,
    simulation_error: Option<UiSimulationError>,
}

#[derive(Deserialize, Debug, Clone)]
struct PubkeyInternal(#[serde(with = "field_as_string")] Pubkey);

impl From<SwapInstructionsResponseInternal> for SwapInstructionsResponse {
    fn from(v: SwapInstructionsResponseInternal) -> Self {
        Self {
            token_ledger_instruction: v.token_ledger_instruction.map(Into::into),
            compute_budget_instructions: v
                .compute_budget_instructions
                .into_iter()
                .map(Into::into)
                .collect(),
            setup_instructions: v.setup_instructions.into_iter().map(Into::into).collect(),
            swap_instruction: v.swap_instruction.into(),
            cleanup_instruction: v.cleanup_instruction.map(Into::into),
            other_instructions: v.other_instructions.into_iter().map(Into::into).collect(),
            address_lookup_table_addresses: v
                .address_lookup_table_addresses
                .into_iter()
                .map(|p| p.0)
                .collect(),
            prioritization_fee_lamports: v.prioritization_fee_lamports,
            compute_unit_limit: v.compute_unit_limit,
            prioritization_type: v.prioritization_type,
            dynamic_slippage_report: v.dynamic_slippage_report,
            simulation_error: v.simulation_error,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    pub route_plan: RoutePlanWithMetadata,
    #[serde(with = "field_as_string")]
    pub input_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub output_mint: Pubkey,
    #[serde(with = "field_as_string")]
    pub in_amount: u64,
    #[serde(with = "field_as_string")]
    pub out_amount: u64,
    #[serde(with = "field_as_string")]
    pub other_amount_threshold: u64,
    pub swap_mode: SwapMode,
    pub transaction: String,
    pub request_id: String,
    pub error_message: Option<String>,
    pub router: String,
    pub slippage_bps: u64,
}

#[derive(Clone)]
struct JupiterClientRef {
    client: Client,
    base_path: String,
    api_key: Option<String>,
}

#[derive(Clone)]
pub struct JupiterClient {
    inner: Arc<JupiterClientRef>,
}

async fn check_is_success(response: reqwest::Response) -> Result<reqwest::Response> {
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.ok();
        return Err(anyhow!("Request failed: {}, body: {:?}", status, text));
    }

    Ok(response)
}

async fn check_and_deserialize<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    check_is_success(response)
        .await?
        .json::<T>()
        .await
        .map_err(Into::into)
}

impl JupiterClient {
    fn build_inner(base_path: impl AsRef<str>, client: Client, api_key: Option<String>) -> Self {
        Self {
            inner: Arc::new(JupiterClientRef {
                client,
                base_path: base_path.as_ref().to_string(),
                api_key,
            }),
        }
    }

    pub fn new(base_path: impl AsRef<str>) -> anyhow::Result<Self> {
        Ok(Self::build_inner(
            base_path,
            ClientBuilder::new().build()?,
            None,
        ))
    }

    pub fn new_with_apikey(
        base_path: impl AsRef<str>,
        api_key: impl AsRef<str>,
    ) -> anyhow::Result<Self> {
        Ok(Self::build_inner(
            base_path,
            ClientBuilder::new().build()?,
            Some(api_key.as_ref().to_string()),
        ))
    }

    pub fn new_with_timeout(base_path: impl AsRef<str>, timeout: Duration) -> anyhow::Result<Self> {
        let client = ClientBuilder::new().timeout(timeout).build()?;

        Ok(Self::build_inner(base_path, client, None))
    }

    pub fn new_with_timeout_and_apikey(
        base_path: impl AsRef<str>,
        timeout: Duration,
        api_key: impl AsRef<str>,
    ) -> anyhow::Result<Self> {
        let client = ClientBuilder::new().timeout(timeout).build()?;

        Ok(Self::build_inner(
            base_path,
            client,
            Some(api_key.as_ref().to_string()),
        ))
    }

    pub fn new_with_client(base_path: impl AsRef<str>, client: Client) -> Self {
        Self::build_inner(base_path, client, None)
    }

    pub fn new_with_client_and_apikey(
        base_path: impl AsRef<str>,
        client: Client,
        api_key: impl AsRef<str>,
    ) -> Self {
        Self::build_inner(base_path, client, Some(api_key.as_ref().to_string()))
    }

    pub fn base_path(&self) -> &str {
        &self.inner.base_path
    }

    pub fn api_key(&self) -> Option<&str> {
        self.inner.api_key.as_deref()
    }

    pub async fn request(
        &self,
        method: reqwest::Method,
        path: impl AsRef<str>,
    ) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.inner.base_path, path.as_ref());
        let mut builder = self.inner.client.request(method, &url);

        if let Some(ref api_key) = self.inner.api_key {
            builder = builder.header("x-api-key", api_key);
        }

        Ok(builder.send().await?)
    }

    pub async fn quote(&self, request: &QuoteRequest) -> Result<QuoteResponse> {
        let url = format!("{}/quote", self.inner.base_path);
        let internal = InternalQuoteRequest::from(request.clone());

        let mut builder = self
            .inner
            .client
            .get(&url)
            .query(&internal)
            .query(&request.quote_args);

        if let Some(ref api_key) = self.inner.api_key {
            builder = builder.header("x-api-key", api_key);
        }

        check_and_deserialize(builder.send().await?).await
    }

    pub async fn quote_raw(&self, request: &QuoteRequest) -> Result<reqwest::Response> {
        let url = format!("{}/quote", self.inner.base_path);
        let internal = InternalQuoteRequest::from(request.clone());

        let mut builder = self
            .inner
            .client
            .get(&url)
            .query(&internal)
            .query(&request.quote_args);

        if let Some(ref api_key) = self.inner.api_key {
            builder = builder.header("x-api-key", api_key);
        }

        Ok(builder.send().await?)
    }

    pub async fn swap(
        &self,
        swap_request: &SwapRequest,
        extra_args: Option<HashMap<String, String>>,
    ) -> Result<SwapResponse> {
        let mut builder = self
            .inner
            .client
            .post(format!("{}/swap", self.inner.base_path))
            .query(&extra_args)
            .json(swap_request);

        if let Some(ref api_key) = self.inner.api_key {
            builder = builder.header("x-api-key", api_key);
        }

        check_and_deserialize(builder.send().await?).await
    }

    pub async fn swap_instructions(
        &self,
        swap_request: &SwapRequest,
    ) -> Result<SwapInstructionsResponse> {
        let mut builder = self
            .inner
            .client
            .post(format!("{}/swap-instructions", self.inner.base_path))
            .json(swap_request);

        if let Some(ref api_key) = self.inner.api_key {
            builder = builder.header("x-api-key", api_key);
        }

        check_and_deserialize::<SwapInstructionsResponseInternal>(builder.send().await?)
            .await
            .map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::solana_sdk::pubkey;
    use super::*;

    #[tokio::test]
    async fn test_swap() {
        let client = JupiterClient::new_with_apikey(
            "https://api.jup.ag/swap/v1",
            "6fec26c0-9178-4d63-abe2-e29f8a10107f",
        )
        .unwrap();

        let quote = client
            .quote(&QuoteRequest {
                input_mint: pubkey!("So11111111111111111111111111111111111111112"),
                output_mint: pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
                amount: 1_000_000_000,
                ..Default::default()
            })
            .await
            .unwrap();

        println!("{:#?}", quote);
    }
}
