// SPDX-License-Identifier: AGPL-3.0-or-later
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  listParcels,
  getParcel,
  getEudrValidation,
  getDdsForParcel,
  submitDdsToTracesNt,
  listCooperatives,
} from '../api/client';
import type { ParcelListQuery, Uuid } from '../api/types';

export function useParcels(query: ParcelListQuery = {}) {
  return useQuery({
    queryKey: ['parcels', query],
    queryFn: () => listParcels(query),
  });
}

export function useParcel(id: Uuid | undefined) {
  return useQuery({
    queryKey: ['parcel', id],
    queryFn: () => getParcel(id as Uuid),
    enabled: Boolean(id),
  });
}

export function useEudrValidation(parcelId: Uuid | undefined) {
  return useQuery({
    queryKey: ['eudr-validation', parcelId],
    queryFn: () => getEudrValidation(parcelId as Uuid),
    enabled: Boolean(parcelId),
  });
}

export function useDds(parcelId: Uuid | undefined) {
  return useQuery({
    queryKey: ['dds', parcelId],
    queryFn: () => getDdsForParcel(parcelId as Uuid),
    enabled: Boolean(parcelId),
  });
}

export function useSubmitDds() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (ddsId: Uuid) => submitDdsToTracesNt(ddsId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['dds'] });
    },
  });
}

export function useCooperatives() {
  return useQuery({
    queryKey: ['cooperatives'],
    queryFn: () => listCooperatives(),
    staleTime: 5 * 60_000,
  });
}
