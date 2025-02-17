import Recoilnexus from "recoil-nexus";
import { clientState } from "../../src/utils/atoms";
import { RecoilRoot, useRecoilState, useSetRecoilState } from "recoil";
import React, { PropsWithChildren, useEffect } from "react";
import FakeClient from "./fake_client";
import Theme from "../../src/utils/theme_provider";

function RootRenderer({ children }: PropsWithChildren<any>) {
  const [client, setClient] = useRecoilState(clientState);

  useEffect(() => {
    setClient(new FakeClient());
  }, []);

  return <>{client && children}</>;
}

export default function FakeRoot({ children }: PropsWithChildren<any>) {
  return (
    <RecoilRoot>
      <Recoilnexus />
      <Theme>
        <RootRenderer>{children}</RootRenderer>
      </Theme>
    </RecoilRoot>
  );
}
