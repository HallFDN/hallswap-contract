import { PlainMessage } from "@bufbuild/protobuf";
import { Adapter } from "cosmes/client";
import { CosmwasmWasmV1MsgStoreCode as ProtoMsgStoreCode } from "cosmes/protobufs";

type Data = PlainMessage<ProtoMsgStoreCode>;

export class MsgStoreCode implements Adapter {
  private readonly data: Data;

  constructor(data: Data) {
    this.data = data;
  }

  public toProto() {
    return new ProtoMsgStoreCode(this.data);
  }

  public toAmino() {
    // TODO: implement this
    return {
      type: "",
      value: {},
    };
  }
}
