import { useState } from "react";
import {
  defaultRepresentativeImage,
  useRepresentativeImage,
} from "../representative-image";

export function RepresentativeImage() {
  const source = useRepresentativeImage();
  const [failedSource, setFailedSource] = useState<string>();
  const displayedSource =
    failedSource === source ? defaultRepresentativeImage : source;
  return (
    <img
      src={displayedSource}
      alt=""
      data-custom={displayedSource !== defaultRepresentativeImage}
      onError={() => setFailedSource(source)}
    />
  );
}
