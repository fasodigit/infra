// SPDX-License-Identifier: AGPL-3.0-or-later
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  listProducers,
  getProducer,
  approveKyc,
  suspendProducer,
  resetMfa,
  getParcelsByProducer,
} from '../api/client';
import type { ProducerListQuery, Uuid } from '../api/types';

export function useProducers(query: ProducerListQuery = {}) {
  return useQuery({
    queryKey: ['producers', query],
    queryFn: () => listProducers(query),
  });
}

export function useProducer(id: Uuid | undefined) {
  return useQuery({
    queryKey: ['producer', id],
    queryFn: () => getProducer(id as Uuid),
    enabled: Boolean(id),
  });
}

export function useProducerParcels(producerId: Uuid | undefined) {
  return useQuery({
    queryKey: ['producer-parcels', producerId],
    queryFn: () => getParcelsByProducer(producerId as Uuid),
    enabled: Boolean(producerId),
  });
}

export function useApproveKyc() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Uuid) => approveKyc(id),
    onSuccess: (_, id) => {
      qc.invalidateQueries({ queryKey: ['producer', id] });
      qc.invalidateQueries({ queryKey: ['producers'] });
    },
  });
}

export function useSuspendProducer() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, reason }: { id: Uuid; reason: string }) =>
      suspendProducer(id, reason),
    onSuccess: (_, vars) => {
      qc.invalidateQueries({ queryKey: ['producer', vars.id] });
      qc.invalidateQueries({ queryKey: ['producers'] });
    },
  });
}

export function useResetMfa() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Uuid) => resetMfa(id),
    onSuccess: (_, id) => {
      qc.invalidateQueries({ queryKey: ['producer', id] });
    },
  });
}
