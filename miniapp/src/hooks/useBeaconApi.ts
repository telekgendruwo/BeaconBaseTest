import { useQuery, useMutation } from '@tanstack/react-query';
import { searchAgents, getAgent, generateManifest, validateContent } from '../lib/api';

export function useSearchAgents(query?: string, limit = 20, offset = 0) {
  return useQuery({
    queryKey: ['agents', query, limit, offset],
    queryFn: () => searchAgents(query, limit, offset),
  });
}

export function useAgent(id: string) {
  return useQuery({
    queryKey: ['agent', id],
    queryFn: () => getAgent(id),
    enabled: !!id,
  });
}

export function useGenerate() {
  return useMutation({
    mutationFn: ({
      githubUrl,
      provider,
      paymentHeaders,
    }: {
      githubUrl: string;
      provider?: string;
      paymentHeaders?: Record<string, string>;
    }) => generateManifest(githubUrl, provider, paymentHeaders),
  });
}

export function useValidate() {
  return useMutation({
    mutationFn: (content: string) => validateContent(content),
  });
}
