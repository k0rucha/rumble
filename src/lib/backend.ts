// Rust 側コマンド (src-tauri/src/commands.rs) の呼び出し口。
// コマンド名と引数名を知っているのはこのファイルだけにする。

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Account,
  MailEvent,
  Mailbox,
  MailboxStatus,
  MessageBody,
  MessageFlag,
  MessagePage,
  NewAccount,
  OutgoingMessage,
  SendReport,
} from "./types";

export const listAccounts = () => invoke<Account[]>("list_accounts");

export const addAccount = (account: NewAccount) => invoke<Account>("add_account", { account });

export const removeAccount = (accountId: string) => invoke<void>("remove_account", { accountId });

export const listMailboxes = (accountId: string) =>
  invoke<Mailbox[]>("list_mailboxes", { accountId });

export const mailboxStatus = (accountId: string, mailbox: string) =>
  invoke<MailboxStatus>("mailbox_status", { accountId, mailbox });

export const listMessages = (accountId: string, mailbox: string, offset: number, limit: number) =>
  invoke<MessagePage>("list_messages", { accountId, mailbox, offset, limit });

export const searchMessages = (accountId: string, mailbox: string, query: string) =>
  invoke<MessagePage>("search_messages", { accountId, mailbox, query });

export const getMessage = (accountId: string, mailbox: string, uid: number) =>
  invoke<MessageBody>("get_message", { accountId, mailbox, uid });

export const saveAttachment = (
  accountId: string,
  mailbox: string,
  uid: number,
  attachmentIndex: number,
  destination: string,
) => invoke<void>("save_attachment", { accountId, mailbox, uid, attachmentIndex, destination });

export const setFlag = (
  accountId: string,
  mailbox: string,
  uids: number[],
  flag: MessageFlag,
  enabled: boolean,
) => invoke<void>("set_flag", { accountId, mailbox, uids, flag, enabled });

export const moveMessages = (
  accountId: string,
  mailbox: string,
  uids: number[],
  destination: string,
) => invoke<void>("move_messages", { accountId, mailbox, uids, destination });

export const deleteMessages = (accountId: string, mailbox: string, uids: number[]) =>
  invoke<void>("delete_messages", { accountId, mailbox, uids });

export const sendMessage = (accountId: string, message: OutgoingMessage) =>
  invoke<SendReport>("send_message", { accountId, message });

/** サーバー側の変化 (IDLE で検知) */
export const onMailEvent = (handler: (event: MailEvent) => void): Promise<UnlistenFn> =>
  listen<MailEvent>("mail-event", (event) => handler(event.payload));

/** メール本文中の mailto: リンクが押された */
export const onComposeRequest = (handler: (address: string) => void): Promise<UnlistenFn> =>
  listen<string>("compose-to", (event) => handler(event.payload));
