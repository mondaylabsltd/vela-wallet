/**
 * Per-language resource aggregator (auto-laid-out).
 *
 * Each language merges its core file (common/language/settings) + every
 * per-namespace file into one flat `translation` object. `en` is also the
 * TypeScript key source (see i18next.d.ts). To add a language: create its
 * files and add it to ALL below.
 */
import enCore from './locales/en.json';
import enHome from './locales/en/home.json';
import enSend from './locales/en/send.json';
import enReceive from './locales/en/receive.json';
import enAssets from './locales/en/assets.json';
import enAddToken from './locales/en/addToken.json';
import enTokenDetail from './locales/en/tokenDetail.json';
import enHistory from './locales/en/history.json';
import enOnboarding from './locales/en/onboarding.json';
import enConnect from './locales/en/connect.json';
import enAbout from './locales/en/about.json';
import enClearSigning from './locales/en/clearSigning.json';
import enComponentsTx from './locales/en/componentsTx.json';
import enComponentsUi from './locales/en/componentsUi.json';
import enSettingsModals from './locales/en/settingsModals.json';
import enContacts from './locales/en/contacts.json';
import zhCore from './locales/zh.json';
import zhHome from './locales/zh/home.json';
import zhSend from './locales/zh/send.json';
import zhReceive from './locales/zh/receive.json';
import zhAssets from './locales/zh/assets.json';
import zhAddToken from './locales/zh/addToken.json';
import zhTokenDetail from './locales/zh/tokenDetail.json';
import zhHistory from './locales/zh/history.json';
import zhOnboarding from './locales/zh/onboarding.json';
import zhConnect from './locales/zh/connect.json';
import zhAbout from './locales/zh/about.json';
import zhClearSigning from './locales/zh/clearSigning.json';
import zhComponentsTx from './locales/zh/componentsTx.json';
import zhComponentsUi from './locales/zh/componentsUi.json';
import zhSettingsModals from './locales/zh/settingsModals.json';
import zhContacts from './locales/zh/contacts.json';
import zhTWCore from './locales/zh-TW.json';
import zhTWHome from './locales/zh-TW/home.json';
import zhTWSend from './locales/zh-TW/send.json';
import zhTWReceive from './locales/zh-TW/receive.json';
import zhTWAssets from './locales/zh-TW/assets.json';
import zhTWAddToken from './locales/zh-TW/addToken.json';
import zhTWTokenDetail from './locales/zh-TW/tokenDetail.json';
import zhTWHistory from './locales/zh-TW/history.json';
import zhTWOnboarding from './locales/zh-TW/onboarding.json';
import zhTWConnect from './locales/zh-TW/connect.json';
import zhTWAbout from './locales/zh-TW/about.json';
import zhTWClearSigning from './locales/zh-TW/clearSigning.json';
import zhTWComponentsTx from './locales/zh-TW/componentsTx.json';
import zhTWComponentsUi from './locales/zh-TW/componentsUi.json';
import zhTWSettingsModals from './locales/zh-TW/settingsModals.json';
import zhTWContacts from './locales/zh-TW/contacts.json';
import zhHKCore from './locales/zh-HK.json';
import zhHKHome from './locales/zh-HK/home.json';
import zhHKSend from './locales/zh-HK/send.json';
import zhHKReceive from './locales/zh-HK/receive.json';
import zhHKAssets from './locales/zh-HK/assets.json';
import zhHKAddToken from './locales/zh-HK/addToken.json';
import zhHKTokenDetail from './locales/zh-HK/tokenDetail.json';
import zhHKHistory from './locales/zh-HK/history.json';
import zhHKOnboarding from './locales/zh-HK/onboarding.json';
import zhHKConnect from './locales/zh-HK/connect.json';
import zhHKAbout from './locales/zh-HK/about.json';
import zhHKClearSigning from './locales/zh-HK/clearSigning.json';
import zhHKComponentsTx from './locales/zh-HK/componentsTx.json';
import zhHKComponentsUi from './locales/zh-HK/componentsUi.json';
import zhHKSettingsModals from './locales/zh-HK/settingsModals.json';
import zhHKContacts from './locales/zh-HK/contacts.json';
import jaCore from './locales/ja.json';
import jaHome from './locales/ja/home.json';
import jaSend from './locales/ja/send.json';
import jaReceive from './locales/ja/receive.json';
import jaAssets from './locales/ja/assets.json';
import jaAddToken from './locales/ja/addToken.json';
import jaTokenDetail from './locales/ja/tokenDetail.json';
import jaHistory from './locales/ja/history.json';
import jaOnboarding from './locales/ja/onboarding.json';
import jaConnect from './locales/ja/connect.json';
import jaAbout from './locales/ja/about.json';
import jaClearSigning from './locales/ja/clearSigning.json';
import jaComponentsTx from './locales/ja/componentsTx.json';
import jaComponentsUi from './locales/ja/componentsUi.json';
import jaSettingsModals from './locales/ja/settingsModals.json';
import jaContacts from './locales/ja/contacts.json';
import koCore from './locales/ko.json';
import koHome from './locales/ko/home.json';
import koSend from './locales/ko/send.json';
import koReceive from './locales/ko/receive.json';
import koAssets from './locales/ko/assets.json';
import koAddToken from './locales/ko/addToken.json';
import koTokenDetail from './locales/ko/tokenDetail.json';
import koHistory from './locales/ko/history.json';
import koOnboarding from './locales/ko/onboarding.json';
import koConnect from './locales/ko/connect.json';
import koAbout from './locales/ko/about.json';
import koClearSigning from './locales/ko/clearSigning.json';
import koComponentsTx from './locales/ko/componentsTx.json';
import koComponentsUi from './locales/ko/componentsUi.json';
import koSettingsModals from './locales/ko/settingsModals.json';
import koContacts from './locales/ko/contacts.json';
import viCore from './locales/vi.json';
import viHome from './locales/vi/home.json';
import viSend from './locales/vi/send.json';
import viReceive from './locales/vi/receive.json';
import viAssets from './locales/vi/assets.json';
import viAddToken from './locales/vi/addToken.json';
import viTokenDetail from './locales/vi/tokenDetail.json';
import viHistory from './locales/vi/history.json';
import viOnboarding from './locales/vi/onboarding.json';
import viConnect from './locales/vi/connect.json';
import viAbout from './locales/vi/about.json';
import viClearSigning from './locales/vi/clearSigning.json';
import viComponentsTx from './locales/vi/componentsTx.json';
import viComponentsUi from './locales/vi/componentsUi.json';
import viSettingsModals from './locales/vi/settingsModals.json';
import viContacts from './locales/vi/contacts.json';
import idCore from './locales/id.json';
import idHome from './locales/id/home.json';
import idSend from './locales/id/send.json';
import idReceive from './locales/id/receive.json';
import idAssets from './locales/id/assets.json';
import idAddToken from './locales/id/addToken.json';
import idTokenDetail from './locales/id/tokenDetail.json';
import idHistory from './locales/id/history.json';
import idOnboarding from './locales/id/onboarding.json';
import idConnect from './locales/id/connect.json';
import idAbout from './locales/id/about.json';
import idClearSigning from './locales/id/clearSigning.json';
import idComponentsTx from './locales/id/componentsTx.json';
import idComponentsUi from './locales/id/componentsUi.json';
import idSettingsModals from './locales/id/settingsModals.json';
import idContacts from './locales/id/contacts.json';
import trCore from './locales/tr.json';
import trHome from './locales/tr/home.json';
import trSend from './locales/tr/send.json';
import trReceive from './locales/tr/receive.json';
import trAssets from './locales/tr/assets.json';
import trAddToken from './locales/tr/addToken.json';
import trTokenDetail from './locales/tr/tokenDetail.json';
import trHistory from './locales/tr/history.json';
import trOnboarding from './locales/tr/onboarding.json';
import trConnect from './locales/tr/connect.json';
import trAbout from './locales/tr/about.json';
import trClearSigning from './locales/tr/clearSigning.json';
import trComponentsTx from './locales/tr/componentsTx.json';
import trComponentsUi from './locales/tr/componentsUi.json';
import trSettingsModals from './locales/tr/settingsModals.json';
import trContacts from './locales/tr/contacts.json';
import esMXCore from './locales/es-MX.json';
import esMXHome from './locales/es-MX/home.json';
import esMXSend from './locales/es-MX/send.json';
import esMXReceive from './locales/es-MX/receive.json';
import esMXAssets from './locales/es-MX/assets.json';
import esMXAddToken from './locales/es-MX/addToken.json';
import esMXTokenDetail from './locales/es-MX/tokenDetail.json';
import esMXHistory from './locales/es-MX/history.json';
import esMXOnboarding from './locales/es-MX/onboarding.json';
import esMXConnect from './locales/es-MX/connect.json';
import esMXAbout from './locales/es-MX/about.json';
import esMXClearSigning from './locales/es-MX/clearSigning.json';
import esMXComponentsTx from './locales/es-MX/componentsTx.json';
import esMXComponentsUi from './locales/es-MX/componentsUi.json';
import esMXSettingsModals from './locales/es-MX/settingsModals.json';
import esMXContacts from './locales/es-MX/contacts.json';
import ptBRCore from './locales/pt-BR.json';
import ptBRHome from './locales/pt-BR/home.json';
import ptBRSend from './locales/pt-BR/send.json';
import ptBRReceive from './locales/pt-BR/receive.json';
import ptBRAssets from './locales/pt-BR/assets.json';
import ptBRAddToken from './locales/pt-BR/addToken.json';
import ptBRTokenDetail from './locales/pt-BR/tokenDetail.json';
import ptBRHistory from './locales/pt-BR/history.json';
import ptBROnboarding from './locales/pt-BR/onboarding.json';
import ptBRConnect from './locales/pt-BR/connect.json';
import ptBRAbout from './locales/pt-BR/about.json';
import ptBRClearSigning from './locales/pt-BR/clearSigning.json';
import ptBRComponentsTx from './locales/pt-BR/componentsTx.json';
import ptBRComponentsUi from './locales/pt-BR/componentsUi.json';
import ptBRSettingsModals from './locales/pt-BR/settingsModals.json';
import ptBRContacts from './locales/pt-BR/contacts.json';
import frCore from './locales/fr.json';
import frHome from './locales/fr/home.json';
import frSend from './locales/fr/send.json';
import frReceive from './locales/fr/receive.json';
import frAssets from './locales/fr/assets.json';
import frAddToken from './locales/fr/addToken.json';
import frTokenDetail from './locales/fr/tokenDetail.json';
import frHistory from './locales/fr/history.json';
import frOnboarding from './locales/fr/onboarding.json';
import frConnect from './locales/fr/connect.json';
import frAbout from './locales/fr/about.json';
import frClearSigning from './locales/fr/clearSigning.json';
import frComponentsTx from './locales/fr/componentsTx.json';
import frComponentsUi from './locales/fr/componentsUi.json';
import frSettingsModals from './locales/fr/settingsModals.json';
import frContacts from './locales/fr/contacts.json';
import deCore from './locales/de.json';
import deHome from './locales/de/home.json';
import deSend from './locales/de/send.json';
import deReceive from './locales/de/receive.json';
import deAssets from './locales/de/assets.json';
import deAddToken from './locales/de/addToken.json';
import deTokenDetail from './locales/de/tokenDetail.json';
import deHistory from './locales/de/history.json';
import deOnboarding from './locales/de/onboarding.json';
import deConnect from './locales/de/connect.json';
import deAbout from './locales/de/about.json';
import deClearSigning from './locales/de/clearSigning.json';
import deComponentsTx from './locales/de/componentsTx.json';
import deComponentsUi from './locales/de/componentsUi.json';
import deSettingsModals from './locales/de/settingsModals.json';
import deContacts from './locales/de/contacts.json';
import ruCore from './locales/ru.json';
import ruHome from './locales/ru/home.json';
import ruSend from './locales/ru/send.json';
import ruReceive from './locales/ru/receive.json';
import ruAssets from './locales/ru/assets.json';
import ruAddToken from './locales/ru/addToken.json';
import ruTokenDetail from './locales/ru/tokenDetail.json';
import ruHistory from './locales/ru/history.json';
import ruOnboarding from './locales/ru/onboarding.json';
import ruConnect from './locales/ru/connect.json';
import ruAbout from './locales/ru/about.json';
import ruClearSigning from './locales/ru/clearSigning.json';
import ruComponentsTx from './locales/ru/componentsTx.json';
import ruComponentsUi from './locales/ru/componentsUi.json';
import ruSettingsModals from './locales/ru/settingsModals.json';
import ruContacts from './locales/ru/contacts.json';
import itCore from './locales/it.json';
import itHome from './locales/it/home.json';
import itSend from './locales/it/send.json';
import itReceive from './locales/it/receive.json';
import itAssets from './locales/it/assets.json';
import itAddToken from './locales/it/addToken.json';
import itTokenDetail from './locales/it/tokenDetail.json';
import itHistory from './locales/it/history.json';
import itOnboarding from './locales/it/onboarding.json';
import itConnect from './locales/it/connect.json';
import itAbout from './locales/it/about.json';
import itClearSigning from './locales/it/clearSigning.json';
import itComponentsTx from './locales/it/componentsTx.json';
import itComponentsUi from './locales/it/componentsUi.json';
import itSettingsModals from './locales/it/settingsModals.json';
import itContacts from './locales/it/contacts.json';

export const en = {
  ...enCore,
  ...enHome,
  ...enSend,
  ...enReceive,
  ...enAssets,
  ...enAddToken,
  ...enTokenDetail,
  ...enHistory,
  ...enOnboarding,
  ...enConnect,
  ...enAbout,
  ...enClearSigning,
  ...enComponentsTx,
  ...enComponentsUi,
  ...enSettingsModals,
  ...enContacts,
};

const zh = {
  ...zhCore,
  ...zhHome,
  ...zhSend,
  ...zhReceive,
  ...zhAssets,
  ...zhAddToken,
  ...zhTokenDetail,
  ...zhHistory,
  ...zhOnboarding,
  ...zhConnect,
  ...zhAbout,
  ...zhClearSigning,
  ...zhComponentsTx,
  ...zhComponentsUi,
  ...zhSettingsModals,
  ...zhContacts,
};

const zhTW = {
  ...zhTWCore,
  ...zhTWHome,
  ...zhTWSend,
  ...zhTWReceive,
  ...zhTWAssets,
  ...zhTWAddToken,
  ...zhTWTokenDetail,
  ...zhTWHistory,
  ...zhTWOnboarding,
  ...zhTWConnect,
  ...zhTWAbout,
  ...zhTWClearSigning,
  ...zhTWComponentsTx,
  ...zhTWComponentsUi,
  ...zhTWSettingsModals,
  ...zhTWContacts,
};

const zhHK = {
  ...zhHKCore,
  ...zhHKHome,
  ...zhHKSend,
  ...zhHKReceive,
  ...zhHKAssets,
  ...zhHKAddToken,
  ...zhHKTokenDetail,
  ...zhHKHistory,
  ...zhHKOnboarding,
  ...zhHKConnect,
  ...zhHKAbout,
  ...zhHKClearSigning,
  ...zhHKComponentsTx,
  ...zhHKComponentsUi,
  ...zhHKSettingsModals,
  ...zhHKContacts,
};

const ja = {
  ...jaCore,
  ...jaHome,
  ...jaSend,
  ...jaReceive,
  ...jaAssets,
  ...jaAddToken,
  ...jaTokenDetail,
  ...jaHistory,
  ...jaOnboarding,
  ...jaConnect,
  ...jaAbout,
  ...jaClearSigning,
  ...jaComponentsTx,
  ...jaComponentsUi,
  ...jaSettingsModals,
  ...jaContacts,
};

const ko = {
  ...koCore,
  ...koHome,
  ...koSend,
  ...koReceive,
  ...koAssets,
  ...koAddToken,
  ...koTokenDetail,
  ...koHistory,
  ...koOnboarding,
  ...koConnect,
  ...koAbout,
  ...koClearSigning,
  ...koComponentsTx,
  ...koComponentsUi,
  ...koSettingsModals,
  ...koContacts,
};

const vi = {
  ...viCore,
  ...viHome,
  ...viSend,
  ...viReceive,
  ...viAssets,
  ...viAddToken,
  ...viTokenDetail,
  ...viHistory,
  ...viOnboarding,
  ...viConnect,
  ...viAbout,
  ...viClearSigning,
  ...viComponentsTx,
  ...viComponentsUi,
  ...viSettingsModals,
  ...viContacts,
};

const id = {
  ...idCore,
  ...idHome,
  ...idSend,
  ...idReceive,
  ...idAssets,
  ...idAddToken,
  ...idTokenDetail,
  ...idHistory,
  ...idOnboarding,
  ...idConnect,
  ...idAbout,
  ...idClearSigning,
  ...idComponentsTx,
  ...idComponentsUi,
  ...idSettingsModals,
  ...idContacts,
};

const tr = {
  ...trCore,
  ...trHome,
  ...trSend,
  ...trReceive,
  ...trAssets,
  ...trAddToken,
  ...trTokenDetail,
  ...trHistory,
  ...trOnboarding,
  ...trConnect,
  ...trAbout,
  ...trClearSigning,
  ...trComponentsTx,
  ...trComponentsUi,
  ...trSettingsModals,
  ...trContacts,
};

const esMX = {
  ...esMXCore,
  ...esMXHome,
  ...esMXSend,
  ...esMXReceive,
  ...esMXAssets,
  ...esMXAddToken,
  ...esMXTokenDetail,
  ...esMXHistory,
  ...esMXOnboarding,
  ...esMXConnect,
  ...esMXAbout,
  ...esMXClearSigning,
  ...esMXComponentsTx,
  ...esMXComponentsUi,
  ...esMXSettingsModals,
  ...esMXContacts,
};

const ptBR = {
  ...ptBRCore,
  ...ptBRHome,
  ...ptBRSend,
  ...ptBRReceive,
  ...ptBRAssets,
  ...ptBRAddToken,
  ...ptBRTokenDetail,
  ...ptBRHistory,
  ...ptBROnboarding,
  ...ptBRConnect,
  ...ptBRAbout,
  ...ptBRClearSigning,
  ...ptBRComponentsTx,
  ...ptBRComponentsUi,
  ...ptBRSettingsModals,
  ...ptBRContacts,
};

const fr = {
  ...frCore,
  ...frHome,
  ...frSend,
  ...frReceive,
  ...frAssets,
  ...frAddToken,
  ...frTokenDetail,
  ...frHistory,
  ...frOnboarding,
  ...frConnect,
  ...frAbout,
  ...frClearSigning,
  ...frComponentsTx,
  ...frComponentsUi,
  ...frSettingsModals,
  ...frContacts,
};

const de = {
  ...deCore,
  ...deHome,
  ...deSend,
  ...deReceive,
  ...deAssets,
  ...deAddToken,
  ...deTokenDetail,
  ...deHistory,
  ...deOnboarding,
  ...deConnect,
  ...deAbout,
  ...deClearSigning,
  ...deComponentsTx,
  ...deComponentsUi,
  ...deSettingsModals,
  ...deContacts,
};

const ru = {
  ...ruCore,
  ...ruHome,
  ...ruSend,
  ...ruReceive,
  ...ruAssets,
  ...ruAddToken,
  ...ruTokenDetail,
  ...ruHistory,
  ...ruOnboarding,
  ...ruConnect,
  ...ruAbout,
  ...ruClearSigning,
  ...ruComponentsTx,
  ...ruComponentsUi,
  ...ruSettingsModals,
  ...ruContacts,
};

const it = {
  ...itCore,
  ...itHome,
  ...itSend,
  ...itReceive,
  ...itAssets,
  ...itAddToken,
  ...itTokenDetail,
  ...itHistory,
  ...itOnboarding,
  ...itConnect,
  ...itAbout,
  ...itClearSigning,
  ...itComponentsTx,
  ...itComponentsUi,
  ...itSettingsModals,
  ...itContacts,
};

export const resources = {
  "en": { translation: en },
  "zh": { translation: zh },
  "zh-TW": { translation: zhTW },
  "zh-HK": { translation: zhHK },
  "ja": { translation: ja },
  "ko": { translation: ko },
  "vi": { translation: vi },
  "id": { translation: id },
  "tr": { translation: tr },
  "es-MX": { translation: esMX },
  "pt-BR": { translation: ptBR },
  "fr": { translation: fr },
  "de": { translation: de },
  "ru": { translation: ru },
  "it": { translation: it },
};
