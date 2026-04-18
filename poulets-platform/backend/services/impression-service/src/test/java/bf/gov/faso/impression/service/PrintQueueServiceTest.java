package bf.gov.faso.impression.service;

import bf.gov.faso.impression.service.impl.PrintQueueServiceImpl;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.data.redis.core.ZSetOperations;

import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class PrintQueueServiceTest {

    @Mock
    private StringRedisTemplate redisTemplate;

    @Mock
    private ZSetOperations<String, String> zSetOps;

    private PrintQueueService printQueueService;

    private static final String TENANT_ID = "test-tenant";
    private static final String QUEUE_KEY = "print:queue:" + TENANT_ID;

    @BeforeEach
    void setUp() {
        when(redisTemplate.opsForZSet()).thenReturn(zSetOps);
        printQueueService = new PrintQueueServiceImpl(redisTemplate);
    }

    @Test
    @DisplayName("Should add job to queue with correct score")
    void shouldAddJobToQueueWithCorrectScore() {
        // Given
        UUID printJobId = UUID.randomUUID();
        int priority = 3;

        when(zSetOps.add(anyString(), anyString(), anyDouble())).thenReturn(true);

        // When
        printQueueService.addToQueue(printJobId, TENANT_ID, priority);

        // Then
        verify(zSetOps).add(eq(QUEUE_KEY), eq(printJobId.toString()), anyDouble());
    }

    @Test
    @DisplayName("Should remove job from queue")
    void shouldRemoveJobFromQueue() {
        // Given
        UUID printJobId = UUID.randomUUID();
        when(zSetOps.remove(QUEUE_KEY, printJobId.toString())).thenReturn(1L);

        // When
        boolean removed = printQueueService.removeFromQueue(printJobId, TENANT_ID);

        // Then
        assertThat(removed).isTrue();
        verify(zSetOps).remove(QUEUE_KEY, printJobId.toString());
    }

    @Test
    @DisplayName("Should return false when removing non-existent job")
    void shouldReturnFalseWhenRemovingNonExistentJob() {
        // Given
        UUID printJobId = UUID.randomUUID();
        when(zSetOps.remove(QUEUE_KEY, printJobId.toString())).thenReturn(0L);

        // When
        boolean removed = printQueueService.removeFromQueue(printJobId, TENANT_ID);

        // Then
        assertThat(removed).isFalse();
    }

    @Test
    @DisplayName("Should get next batch from queue")
    void shouldGetNextBatchFromQueue() {
        // Given
        UUID job1 = UUID.randomUUID();
        UUID job2 = UUID.randomUUID();
        Set<String> batch = new LinkedHashSet<>();
        batch.add(job1.toString());
        batch.add(job2.toString());

        when(zSetOps.rangeByScore(eq(QUEUE_KEY), eq(0.0), eq(Double.MAX_VALUE), eq(0L), eq(10L)))
            .thenReturn(batch);

        // When
        List<UUID> result = printQueueService.getNextBatch(TENANT_ID, 10);

        // Then
        assertThat(result).hasSize(2);
        assertThat(result).containsExactly(job1, job2);
    }

    @Test
    @DisplayName("Should return empty list when queue is empty")
    void shouldReturnEmptyListWhenQueueIsEmpty() {
        // Given
        when(zSetOps.rangeByScore(eq(QUEUE_KEY), anyDouble(), anyDouble(), anyLong(), anyLong()))
            .thenReturn(null);

        // When
        List<UUID> result = printQueueService.getNextBatch(TENANT_ID, 10);

        // Then
        assertThat(result).isEmpty();
    }

    @Test
    @DisplayName("Should get queue position")
    void shouldGetQueuePosition() {
        // Given
        UUID printJobId = UUID.randomUUID();
        when(zSetOps.rank(QUEUE_KEY, printJobId.toString())).thenReturn(3L);

        // When
        long position = printQueueService.getQueuePosition(printJobId, TENANT_ID);

        // Then
        assertThat(position).isEqualTo(3L);
    }

    @Test
    @DisplayName("Should return -1 for non-existent job position")
    void shouldReturnMinusOneForNonExistentJobPosition() {
        // Given
        UUID printJobId = UUID.randomUUID();
        when(zSetOps.rank(QUEUE_KEY, printJobId.toString())).thenReturn(null);

        // When
        long position = printQueueService.getQueuePosition(printJobId, TENANT_ID);

        // Then
        assertThat(position).isEqualTo(-1L);
    }

    @Test
    @DisplayName("Should get queue size")
    void shouldGetQueueSize() {
        // Given
        when(zSetOps.size(QUEUE_KEY)).thenReturn(5L);

        // When
        long size = printQueueService.getQueueSize(TENANT_ID);

        // Then
        assertThat(size).isEqualTo(5L);
    }

    @Test
    @DisplayName("Should update priority")
    void shouldUpdatePriority() {
        // Given
        UUID printJobId = UUID.randomUUID();
        double currentScore = 5_000_000_000.0 + 123456789; // Priority 5 + timestamp
        when(zSetOps.score(QUEUE_KEY, printJobId.toString())).thenReturn(currentScore);
        when(zSetOps.add(anyString(), anyString(), anyDouble())).thenReturn(true);

        // When
        boolean updated = printQueueService.updatePriority(printJobId, TENANT_ID, 2);

        // Then
        assertThat(updated).isTrue();
        verify(zSetOps).add(eq(QUEUE_KEY), eq(printJobId.toString()), anyDouble());
    }

    @Test
    @DisplayName("Should move job to front of queue")
    void shouldMoveJobToFront() {
        // Given
        UUID printJobId = UUID.randomUUID();
        when(zSetOps.remove(QUEUE_KEY, printJobId.toString())).thenReturn(1L);
        when(zSetOps.add(QUEUE_KEY, printJobId.toString(), 0.0)).thenReturn(true);

        // When
        printQueueService.moveToFront(printJobId, TENANT_ID);

        // Then
        verify(zSetOps).remove(QUEUE_KEY, printJobId.toString());
        verify(zSetOps).add(QUEUE_KEY, printJobId.toString(), 0.0);
    }

    @Test
    @DisplayName("Should check if job is in queue")
    void shouldCheckIfJobIsInQueue() {
        // Given
        UUID printJobId = UUID.randomUUID();
        when(zSetOps.score(QUEUE_KEY, printJobId.toString())).thenReturn(5.0);

        // When
        boolean inQueue = printQueueService.isInQueue(printJobId, TENANT_ID);

        // Then
        assertThat(inQueue).isTrue();
    }

    @Test
    @DisplayName("Should return false for job not in queue")
    void shouldReturnFalseForJobNotInQueue() {
        // Given
        UUID printJobId = UUID.randomUUID();
        when(zSetOps.score(QUEUE_KEY, printJobId.toString())).thenReturn(null);

        // When
        boolean inQueue = printQueueService.isInQueue(printJobId, TENANT_ID);

        // Then
        assertThat(inQueue).isFalse();
    }
}
