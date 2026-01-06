import { QueryClient, QueryFunction } from "@tanstack/react-query";

async function throwIfResNotOk(res: Response) {
  if (!res.ok) {
    const text = (await res.text()) || res.statusText;
    throw new Error(`${res.status}: ${text}`);
  }
}

export async function apiRequest(
  method: string,
  url: string,
  data?: unknown | undefined,
): Promise<Response> {
  const res = await fetch(url, {
    method,
    headers: data ? { "Content-Type": "application/json" } : {},
    body: data ? JSON.stringify(data) : undefined,
    credentials: "include",
  });

  await throwIfResNotOk(res);
  return res;
}

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5, // 5 minutes
      retry: 1,
    },
  },
});

export const createQueryFn =
  <T>({ on401: unauthorizedBehavior }: { on401: "redirect" | "throw" }) =>
  async ({ queryKey }: { queryKey: readonly unknown[] }) => {
    const res = await fetch(queryKey[0] as string, {
      credentials: "include",
    });

    if (res.status === 401) {
      if (unauthorizedBehavior === "redirect") {
        window.location.href = "/login";
        return;
      }
      throw new Error("Unauthorized");
    }

    if (!res.ok) {
      throw new Error("Network response was not ok");
    }

    return res.json() as Promise<T>;
  };
