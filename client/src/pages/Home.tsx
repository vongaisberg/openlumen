import { useEffect } from "react";
import ArtNetNode from "@/components/ArtNetNode";
import { Helmet } from "react-helmet";

export default function Home() {
  useEffect(() => {
    // Set document metadata
    document.title = "ArtNet Node Controller";
  }, []);

  return (
    <>
      <Helmet>
        <meta name="description" content="Configure and monitor an ArtNet node with 4 DMX ports. Manage network settings, ArtNet configuration and DMX port controls." />
        <meta property="og:title" content="ArtNet Node Controller" />
        <meta property="og:description" content="Configure and monitor an ArtNet node with 4 DMX ports" />
        <meta property="og:type" content="website" />
      </Helmet>
      <ArtNetNode />
    </>
  );
}
