export class AssetInfo {
  public readonly id: string;

  public constructor(id: string) {
    this.id = id;
  }

  public isCW20() {
    return this.id.length === 64 && this.id.startsWith("terra1");
  }

  public toJSON():
    | { token: { contract_addr: string } }
    | { native_token: { denom: string } } {
    if (this.isCW20()) {
      return {
        token: {
          contract_addr: this.id,
        },
      };
    } else {
      return {
        native_token: {
          denom: this.id,
        },
      };
    }
  }

  public static fromJSON(json: any): AssetInfo {
    if ("token" in json) {
      return new AssetInfo(json.token.contract_addr);
    }
    if ("native_token" in json) {
      return new AssetInfo(json.native_token.denom);
    }
    throw new Error(`unable to parse AssetInfo: ${json}`);
  }
}
