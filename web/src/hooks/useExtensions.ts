import { useEffect, useState } from "react";
import { useRecoilValue } from "recoil";
import { ManifestInfo } from "../types/client";
import { clientState } from "../utils/atoms";

/**
 * Easily retrieve the extensions using the client
 *
 * @returns All the loaded extensions manifests
 */
export default function useExtensions() {
  const [extensions, setExtensions] = useState<ManifestInfo[]>([]);
  const client = useRecoilValue(clientState);

  useEffect(() => {
    client.get_ext_list_by_id().then(async (response) => {
      if (response.Ok) {
        const extesions_list = response.Ok;
        const extensions_info_list = await Promise.all(
          extesions_list.map(async (id) => {
            const info_response = await client.get_ext_info_by_id(id);
            return info_response.Ok as ManifestInfo;
          })
        );
        setExtensions(extensions_info_list);
      }
    });
  }, []);

  return extensions;
}
