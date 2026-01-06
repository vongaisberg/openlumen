declare module 'vite-plugin-bundle-analyzer' {
  export interface BundleAnalyzerOptions {
    open?: boolean;
    filename?: string;
  }
 
  export function bundleAnalyzer(options?: BundleAnalyzerOptions): any;
} 