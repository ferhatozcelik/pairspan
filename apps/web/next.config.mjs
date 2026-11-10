/** Static export: `next build` writes out/, which the Rust gateway serves (see Dockerfile). */
const config = {
  output: 'export',
  trailingSlash: true,
  images: { unoptimized: true },
  reactStrictMode: true,
};
export default config;
