import { PlainMessage } from "@bufbuild/protobuf";
import { Adapter } from "cosmes/client";
import { CosmwasmWasmV1MsgInstantiateContract as ProtoMsgInstantiateContract } from "cosmes/protobufs";

type Data = PlainMessage<ProtoMsgInstantiateContract>;

export class MsgInstantiateContract implements Adapter {
  private readonly data: Data;

  constructor(data: Data) {
    this.data = data;
  }

  public toProto() {
    return new ProtoMsgInstantiateContract(this.data);
  }

  public toAmino() {
    // TODO: implement this
    return {
      type: "",
      value: {},
    };
  }
}
