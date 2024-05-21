import { AssetInfo } from "./AssetInfo";

export class Asset {
  public readonly assetInfo: AssetInfo;
  public readonly amount: bigint;

  public constructor(id: string, amount: string | bigint) {
    this.assetInfo = new AssetInfo(id);
    this.amount = BigInt(amount);
  }

  public get id(): string {
    return this.assetInfo.id;
  }

  public isCW20() {
    return this.assetInfo.isCW20();
  }

  public toJSON() {
    return {
      info: this.assetInfo.toJSON(),
      amount: this.amount.toString(),
    };
  }

  public static fromJSON(json: any) {
    const { info, amount } = json;
    return new Asset(AssetInfo.fromJSON(info).id, amount);
  }
}
