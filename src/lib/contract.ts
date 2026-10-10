// Compile-time contract between src/lib/types.ts and the Rust serde structs.
//
// `types.ts` is hand-written on purpose — it carries docs and narrower types
// (GpuGen, the folder kind union) the Rust side can't express — so nothing
// generates it. Instead `cargo test` writes each IPC struct's shape to
// src/lib/generated/ (ts-rs, test-only, see the cfg_attr on each struct), and
// this file makes `pnpm check` fail when a hand-written interface gains,
// loses or renames a field the Rust struct doesn't have, or an enum's string
// values differ. Field *types* aren't compared: ts-rs spells u64 as bigint
// and Rust Strings as string where types.ts narrows them.
//
// After changing a struct in ipc.rs & co: `cd src-tauri && cargo test`, then
// `pnpm check`. CI checks the generated files are committed up to date.

import type * as T from "./types";
import type { AntiCheat as R_AntiCheat } from "./generated/AntiCheat";
import type { Bootstrap as R_Bootstrap } from "./generated/Bootstrap";
import type { Catalog as R_Catalog } from "./generated/Catalog";
import type { Change as R_Change } from "./generated/Change";
import type { ChangeKind as R_ChangeKind } from "./generated/ChangeKind";
import type { Channel as R_Channel } from "./generated/Channel";
import type { Config as R_Config } from "./generated/Config";
import type { ConfigWarning as R_ConfigWarning } from "./generated/ConfigWarning";
import type { DiffStatus as R_DiffStatus } from "./generated/DiffStatus";
import type { EnvDef as R_EnvDef } from "./generated/EnvDef";
import type { ExportResult as R_ExportResult } from "./generated/ExportResult";
import type { Fix as R_Fix } from "./generated/Fix";
import type { Folder as R_Folder } from "./generated/Folder";
import type { GameDto as R_GameDto } from "./generated/GameDto";
import type { Hardware as R_Hardware } from "./generated/Hardware";
import type { InjectResult as R_InjectResult } from "./generated/InjectResult";
import type { LaunchDiff as R_LaunchDiff } from "./generated/LaunchDiff";
import type { LlmChange as R_LlmChange } from "./generated/LlmChange";
import type { LlmRequest as R_LlmRequest } from "./generated/LlmRequest";
import type { LlmSuggestion as R_LlmSuggestion } from "./generated/LlmSuggestion";
import type { LogSource as R_LogSource } from "./generated/LogSource";
import type { LsfgLayer as R_LsfgLayer } from "./generated/LsfgLayer";
import type { LsfgProfile as R_LsfgProfile } from "./generated/LsfgProfile";
import type { LsfgStatus as R_LsfgStatus } from "./generated/LsfgStatus";
import type { Meta as R_Meta } from "./generated/Meta";
import type { Monitor as R_Monitor } from "./generated/Monitor";
import type { Notice as R_Notice } from "./generated/Notice";
import type { OptiscalerExtractResult as R_OptiscalerExtractResult } from "./generated/OptiscalerExtractResult";
import type { OptiscalerRelease as R_OptiscalerRelease } from "./generated/OptiscalerRelease";
import type { OptiscalerStatus as R_OptiscalerStatus } from "./generated/OptiscalerStatus";
import type { ParsedCommand as R_ParsedCommand } from "./generated/ParsedCommand";
import type { Paths as R_Paths } from "./generated/Paths";
import type { Preset as R_Preset } from "./generated/Preset";
import type { ProtonLog as R_ProtonLog } from "./generated/ProtonLog";
import type { Recipe as R_Recipe } from "./generated/Recipe";
import type { RecipeChange as R_RecipeChange } from "./generated/RecipeChange";
import type { RecipeKind as R_RecipeKind } from "./generated/RecipeKind";
import type { Recompute as R_Recompute } from "./generated/Recompute";
import type { RuntimeDto as R_RuntimeDto } from "./generated/RuntimeDto";
import type { RuntimeUpdate as R_RuntimeUpdate } from "./generated/RuntimeUpdate";
import type { Severity as R_Severity } from "./generated/Severity";
import type { StaleInfo as R_StaleInfo } from "./generated/StaleInfo";
import type { SteamUserConfig as R_SteamUserConfig } from "./generated/SteamUserConfig";
import type { Store as R_Store } from "./generated/Store";
import type { Tier as R_Tier } from "./generated/Tier";
import type { Token as R_Token } from "./generated/Token";
import type { TokenKind as R_TokenKind } from "./generated/TokenKind";
import type { TroubleshootRequest as R_TroubleshootRequest } from "./generated/TroubleshootRequest";
import type { TroubleshootResult as R_TroubleshootResult } from "./generated/TroubleshootResult";
import type { UpdateInfo as R_UpdateInfo } from "./generated/UpdateInfo";
import type { WarningKind as R_WarningKind } from "./generated/WarningKind";
import type { WrapperDef as R_WrapperDef } from "./generated/WrapperDef";
import type { WrapperKind as R_WrapperKind } from "./generated/WrapperKind";

type Keys<A, B> = [Exclude<keyof A, keyof B>, Exclude<keyof B, keyof A>] extends [never, never]
  ? true
  : { onlyInTypesTs: Exclude<keyof A, keyof B>; onlyInRust: Exclude<keyof B, keyof A> };
type Union<A, B> = [A] extends [B] ? ([B] extends [A] ? true : { onlyInRust: Exclude<B, A> }) : { onlyInTypesTs: Exclude<A, B> };
// A failing pair surfaces as "Type '{ onlyIn…: … }' does not satisfy the constraint 'true'".
type Same<_ extends true> = never;

export type _Contract = [
  Same<Keys<T.WrapperDef, R_WrapperDef>>,
  Same<Keys<T.EnvDef, R_EnvDef>>,
  Same<Keys<T.Meta, R_Meta>>,
  Same<Keys<T.Catalog, R_Catalog>>,
  Same<Keys<T.Recipe, R_Recipe>>,
  Same<Keys<T.Hardware, R_Hardware>>,
  Same<Keys<T.Monitor, R_Monitor>>,
  Same<Keys<T.RuntimeDto, R_RuntimeDto>>,
  Same<Keys<T.GameDto, R_GameDto>>,
  Same<Keys<T.HeroicInjectResult, R_InjectResult>>,
  Same<Keys<T.StaleInfo, R_StaleInfo>>,
  Same<Keys<T.ProtonLog, R_ProtonLog>>,
  Same<Keys<T.LogSource, R_LogSource>>,
  Same<Keys<T.LlmRequest, R_LlmRequest>>,
  Same<Keys<T.LlmChange, R_LlmChange>>,
  Same<Keys<T.LlmSuggestion, R_LlmSuggestion>>,
  Same<Keys<T.TroubleshootRequest, R_TroubleshootRequest>>,
  Same<Keys<T.TroubleshootResult, R_TroubleshootResult>>,
  Same<Keys<T.UpdateInfo, R_UpdateInfo>>,
  Same<Keys<T.OptiscalerStatus, R_OptiscalerStatus>>,
  Same<Keys<T.OptiscalerRelease, R_OptiscalerRelease>>,
  Same<Keys<T.OptiscalerExtractResult, R_OptiscalerExtractResult>>,
  Same<Keys<T.LsfgLayer, R_LsfgLayer>>,
  Same<Keys<T.LsfgProfile, R_LsfgProfile>>,
  Same<Keys<T.LsfgStatus, R_LsfgStatus>>,
  Same<Keys<T.RuntimeUpdate, R_RuntimeUpdate>>,
  Same<Keys<T.MangohudExportResult, R_ExportResult>>,
  Same<Keys<T.VkBasaltExportResult, R_ExportResult>>,
  Same<Keys<T.Config, R_Config>>,
  Same<Keys<T.ParsedCommand, R_ParsedCommand>>,
  Same<Keys<T.Preset, R_Preset>>,
  Same<Keys<T.Paths, R_Paths>>,
  Same<Keys<T.Store, R_Store>>,
  Same<Keys<T.Token, R_Token>>,
  Same<Keys<T.LintFix, R_Fix>>,
  Same<Keys<T.Notice, R_Notice>>,
  Same<Keys<T.RecipeChange, R_RecipeChange>>,
  Same<Keys<T.Change, R_Change>>,
  Same<Keys<T.LaunchDiff, R_LaunchDiff>>,
  Same<Keys<T.Recompute, R_Recompute>>,
  Same<Keys<T.Tier, R_Tier>>,
  Same<Keys<T.ConfigWarning, R_ConfigWarning>>,
  Same<Keys<T.SteamUserConfig, R_SteamUserConfig>>,
  Same<Keys<T.Bootstrap, R_Bootstrap>>,
  Same<Keys<T.GameFolder, R_Folder>>,
  Same<Keys<T.AntiCheat, R_AntiCheat>>,
  Same<Union<T.WrapperKind, R_WrapperKind>>,
  Same<Union<T.RecipeKind, R_RecipeKind>>,
  Same<Union<T.TokenKind, R_TokenKind>>,
  Same<Union<T.Severity, R_Severity>>,
  Same<Union<T.ChangeKind, R_ChangeKind>>,
  Same<Union<T.DiffStatus, R_DiffStatus>>,
  Same<Union<T.WarningKind, R_WarningKind>>,
  Same<Union<T.OptiscalerChannel, R_Channel>>,
];
