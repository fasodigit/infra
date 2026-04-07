package bf.gov.faso.poulets.model;

import jakarta.persistence.*;
import java.time.Instant;
import java.util.UUID;

@Entity
@Table(name = "commande_items")
public class CommandeItem {

    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    private UUID id;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "commande_id", nullable = false)
    private Commande commande;

    @Column(name = "commande_id", insertable = false, updatable = false)
    private UUID commandeId;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "poulet_id", nullable = false)
    private Poulet poulet;

    @Column(name = "poulet_id", insertable = false, updatable = false)
    private UUID pouletId;

    @Column(nullable = false)
    private int quantity;

    @Column(name = "unit_price", nullable = false)
    private double unitPrice;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt = Instant.now();

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
    }

    // --- Getters and Setters ---

    public UUID getId() { return id; }
    public void setId(UUID id) { this.id = id; }

    public Commande getCommande() { return commande; }
    public void setCommande(Commande commande) { this.commande = commande; }

    public UUID getCommandeId() { return commandeId; }

    public Poulet getPoulet() { return poulet; }
    public void setPoulet(Poulet poulet) { this.poulet = poulet; }

    public UUID getPouletId() { return pouletId; }

    public int getQuantity() { return quantity; }
    public void setQuantity(int quantity) { this.quantity = quantity; }

    public double getUnitPrice() { return unitPrice; }
    public void setUnitPrice(double unitPrice) { this.unitPrice = unitPrice; }

    public Instant getCreatedAt() { return createdAt; }
}
